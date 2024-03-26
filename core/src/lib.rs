use anyhow::{bail, Error, Result};
use bytes::Bytes;
use move_core_types::account_address::AccountAddress;
use move_core_types::effects::{AccountChangeSet, ChangeSet, Op};
use move_core_types::identifier::Identifier;
use move_core_types::language_storage::{ModuleId, StructTag, TypeTag};
use move_core_types::metadata::Metadata;
use move_core_types::resolver::{resource_size, ModuleResolver, ResourceResolver};
use move_core_types::value::MoveTypeLayout;
use risc0_smt::{Key, Smt, Value};
use risc0_zkvm::declare_syscall;
use risc0_zkvm::guest::env;
use risc0_zkvm::{guest::sha, sha::Sha256};
use serde::{Deserialize, Serialize};
use std::collections::{btree_map, BTreeMap};
use std::fmt::Debug;

pub mod codec;

declare_syscall!(pub SYS_ACCOUNT_LOOKUP);

// TODO: define `SignedTransaction`.
// TODO: think more carefully about the transaction layout (e.g. transactions should be Ethereum-compatible?)
#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
    pub payload: TransactionPayload,
    // TODO: other fields (sender, nonce, gas limit, etc)
}

impl Transaction {
    pub fn to_bytes(&self) -> anyhow::Result<Vec<u8>> {
        // TODO: more efficient serialization than JSON
        let bytes = serde_json::to_vec(self)?;
        Ok(bytes)
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum TransactionPayload {
    EntryFunction(EntryFunction),
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct EntryFunction {
    pub module: ModuleId,
    pub function: Identifier,
    pub ty_args: Vec<TypeTag>,
    pub args: Vec<Vec<u8>>,
}

/// Simple in-memory storage for modules and resources under an account.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InMemoryAccountStorage {
    pub resources: BTreeMap<StructTag, Bytes>,
    pub modules: BTreeMap<Identifier, Bytes>,
}

impl InMemoryAccountStorage {
    pub fn to_bytes(&self) -> anyhow::Result<Vec<u8>> {
        // TODO: more efficient serialization than JSON
        let bytes = serde_json::to_vec(self)?;
        Ok(bytes)
    }

    pub fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let result = serde_json::from_slice(&bytes)?;
        Ok(result)
    }
}

/// Simple in-memory storage that can be used as a Move VM storage backend for testing purposes.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InMemoryStorage {
    pub accounts: BTreeMap<AccountAddress, InMemoryAccountStorage>,
}

fn apply_changes<K, V>(
    map: &mut BTreeMap<K, V>,
    changes: impl IntoIterator<Item = (K, Op<V>)>,
) -> Result<()>
where
    K: Ord + Debug,
{
    use btree_map::Entry::*;
    use Op::*;

    for (k, op) in changes.into_iter() {
        match (map.entry(k), op) {
            (Occupied(entry), New(_)) => {
                bail!(
                    "Failed to apply changes -- key {:?} already exists",
                    entry.key()
                )
            }
            (Occupied(entry), Delete) => {
                entry.remove();
            }
            (Occupied(entry), Modify(val)) => {
                *entry.into_mut() = val;
            }
            (Vacant(entry), New(val)) => {
                entry.insert(val);
            }
            (Vacant(entry), Delete | Modify(_)) => bail!(
                "Failed to apply changes -- key {:?} does not exist",
                entry.key()
            ),
        }
    }
    Ok(())
}

fn get_or_insert<K, V, F>(map: &mut BTreeMap<K, V>, key: K, make_val: F) -> &mut V
where
    K: Ord,
    F: FnOnce() -> V,
{
    use btree_map::Entry::*;

    match map.entry(key) {
        Occupied(entry) => entry.into_mut(),
        Vacant(entry) => entry.insert(make_val()),
    }
}

impl InMemoryAccountStorage {
    fn apply(&mut self, account_changeset: AccountChangeSet) -> Result<()> {
        let (modules, resources) = account_changeset.into_inner();
        apply_changes(&mut self.modules, modules)?;
        apply_changes(&mut self.resources, resources)?;
        Ok(())
    }

    fn new() -> Self {
        Self {
            modules: BTreeMap::new(),
            resources: BTreeMap::new(),
        }
    }
}

impl InMemoryStorage {
    pub fn apply_extended(&mut self, changeset: ChangeSet) -> Result<()> {
        for (addr, account_changeset) in changeset.into_inner() {
            match self.accounts.entry(addr) {
                btree_map::Entry::Occupied(entry) => {
                    entry.into_mut().apply(account_changeset)?;
                }
                btree_map::Entry::Vacant(entry) => {
                    let mut account_storage = InMemoryAccountStorage::new();
                    account_storage.apply(account_changeset)?;
                    entry.insert(account_storage);
                }
            }
        }
        Ok(())
    }

    pub fn apply(&mut self, changeset: ChangeSet) -> Result<()> {
        self.apply_extended(changeset)
    }

    pub fn new() -> Self {
        Self {
            accounts: BTreeMap::new(),
        }
    }

    pub fn publish_or_overwrite_module(&mut self, module_id: ModuleId, blob: Vec<u8>) {
        let account = get_or_insert(&mut self.accounts, *module_id.address(), || {
            InMemoryAccountStorage::new()
        });
        account
            .modules
            .insert(module_id.name().to_owned(), blob.into());
    }

    pub fn publish_or_overwrite_resource(
        &mut self,
        addr: AccountAddress,
        struct_tag: StructTag,
        blob: Vec<u8>,
    ) {
        let account = get_or_insert(&mut self.accounts, addr, InMemoryAccountStorage::new);
        account.resources.insert(struct_tag, blob.into());
    }
}

impl ModuleResolver for InMemoryStorage {
    fn get_module_metadata(&self, _module_id: &ModuleId) -> Vec<Metadata> {
        vec![]
    }

    fn get_module(&self, module_id: &ModuleId) -> Result<Option<Bytes>, Error> {
        if let Some(account_storage) = self.accounts.get(module_id.address()) {
            return Ok(account_storage.modules.get(module_id.name()).cloned());
        }
        Ok(None)
    }
}

impl ResourceResolver for InMemoryStorage {
    fn get_resource_bytes_with_metadata_and_layout(
        &self,
        address: &AccountAddress,
        tag: &StructTag,
        _metadata: &[Metadata],
        _maybe_layout: Option<&MoveTypeLayout>,
    ) -> Result<(Option<Bytes>, usize)> {
        if let Some(account_storage) = self.accounts.get(address) {
            let buf = account_storage.resources.get(tag).cloned();
            let buf_size = resource_size(&buf);
            return Ok((buf, buf_size));
        }
        Ok((None, 0))
    }
}

pub struct SmtStorage<'a> {
    smt: &'a Smt,
}

impl<'a> SmtStorage<'a> {
    pub fn new(smt: &'a Smt) -> Self {
        Self { smt }
    }

    fn get_account<H: Sha256>(&self, address: &AccountAddress) -> Option<InMemoryAccountStorage> {
        let key_bytes = H::hash_bytes(address.as_ref());
        let key = {
            let mut buf = [0u32; 8];
            buf.copy_from_slice(key_bytes.as_words());
            Key(buf)
        };
        let (value, proof) = self.smt.get(&key);
        if !proof.verify::<H>(&key, &value, self.smt.get_root()) {
            panic!("Dishonest SMT");
        }
        if value == Value::EMPTY {
            return None;
        }

        let to_host: Vec<u8> = value.0.into_iter().flat_map(|x| x.to_le_bytes()).collect();
        let account_bytes: &[u8] = env::send_recv_slice(SYS_ACCOUNT_LOOKUP, &to_host);
        if H::hash_bytes(account_bytes).as_words() != &value.0 {
            panic!("Dishonest value from host");
        }
        InMemoryAccountStorage::try_from_bytes(account_bytes).ok()
    }
}

impl<'a> ModuleResolver for SmtStorage<'a> {
    fn get_module_metadata(&self, _module_id: &ModuleId) -> Vec<Metadata> {
        vec![]
    }

    fn get_module(&self, module_id: &ModuleId) -> Result<Option<Bytes>, Error> {
        if let Some(account_storage) = self.get_account::<sha::Impl>(module_id.address()) {
            return Ok(account_storage.modules.get(module_id.name()).cloned());
        }
        Ok(None)
    }
}

impl<'a> ResourceResolver for SmtStorage<'a> {
    fn get_resource_bytes_with_metadata_and_layout(
        &self,
        address: &AccountAddress,
        tag: &StructTag,
        _metadata: &[Metadata],
        _maybe_layout: Option<&MoveTypeLayout>,
    ) -> Result<(Option<Bytes>, usize)> {
        if let Some(account_storage) = self.get_account::<sha::Impl>(address) {
            let buf = account_storage.resources.get(tag).cloned();
            let buf_size = resource_size(&buf);
            return Ok((buf, buf_size));
        }
        Ok((None, 0))
    }
}
