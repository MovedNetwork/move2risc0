use bytes::Bytes;
use move_core_types::{
    effects::{AccountChanges, ChangeSet, Op},
    identifier::Identifier,
    language_storage::StructTag,
};

/// Serialization format:
///   - Length (u32; little-endian)
///   - Sequence of pairs (Address, AccountChanges)
pub fn serialize_changes(changes: &ChangeSet) -> Vec<u8> {
    // TODO: standardize serialization (eg using serde)
    let mut result = Vec::new();
    let accounts = changes.accounts();
    let n = accounts.len() as u32;
    result.extend_from_slice(&n.to_le_bytes());
    for (k, v) in accounts {
        let address_bytes: &[u8] = k.as_ref();
        result.extend_from_slice(address_bytes);
        serialize_account_changes(v, &mut result);
    }
    result
}

/// Serialization format:
///   - Modules length (u32; little-endian)
///   - Sequence of pairs (Identifier, Op)
///   - Resources length
///   - Sequence of pairs (StructTag, Op)
fn serialize_account_changes(changes: &AccountChanges<Bytes, Bytes>, result: &mut Vec<u8>) {
    let modules = changes.modules();
    let resources = changes.resources();

    let n = modules.len() as u32;
    result.extend_from_slice(&n.to_le_bytes());
    for (id, op) in modules {
        serialize_identifier(id, result);
        serialize_op(op, result);
    }

    let n = resources.len() as u32;
    result.extend_from_slice(&n.to_le_bytes());
    for (tag, op) in resources {
        serialize_struct_tag(tag, result);
        serialize_op(op, result);
    }
}

fn serialize_identifier(id: &Identifier, result: &mut Vec<u8>) {
    let n = id.len() as u32;
    result.extend_from_slice(&n.to_le_bytes());
    result.extend_from_slice(id.as_bytes());
}

fn serialize_op(op: &Op<Bytes>, result: &mut Vec<u8>) {
    let (tag, bytes) = match op {
        Op::New(bytes) => (0x00, Some(bytes)),
        Op::Modify(bytes) => (0x01, Some(bytes)),
        Op::Delete => (0x02, None),
    };
    result.push(tag);
    if let Some(bytes) = bytes {
        let n = bytes.len() as u32;
        result.extend_from_slice(&n.to_le_bytes());
        result.extend_from_slice(bytes);
    }
}

/// Serialization format:
///   - First serialize tag as JSON
///   - Length of JSON bytes (u32; little-endian)
///   - JSON bytes
fn serialize_struct_tag(tag: &StructTag, result: &mut Vec<u8>) {
    // TODO: more efficient serialization than JSON
    let json_bytes = serde_json::to_vec(tag).unwrap_or_default();
    let n = json_bytes.len() as u32;
    result.extend_from_slice(&n.to_le_bytes());
    result.extend_from_slice(&json_bytes);
}
