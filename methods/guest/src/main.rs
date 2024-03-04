use risc0_zkvm::guest::env;
use {
    bytes::Bytes,
    move_core_types::{
        account_address::AccountAddress,
        identifier::{IdentStr, Identifier},
        language_storage::ModuleId,
    },
    move_vm_runtime::move_vm::MoveVM,
    move_vm_test_utils::{gas_schedule::GasStatus, InMemoryStorage},
    std::str::FromStr,
};

risc0_zkvm::guest::entry!(main);

fn main() {
    let start = env::cycle_count();

    // Insert the smart contract module into storage backend
    let bytes: Vec<u8> = env::read(); // read module bytes from host
    let move_vm = MoveVM::new(vec![]).unwrap();
    let mut storage = InMemoryStorage::new();
    let module_id = ModuleId::new(AccountAddress::ZERO, Identifier::from_str("add").unwrap());
    storage.publish_or_overwrite_module(module_id.clone(), bytes);

    // Run entry function on the module
    let mut session = move_vm.new_session(&storage);
    let mut gas_meter = GasStatus::new_unmetered();
    let result = session.execute_entry_function(
        &module_id,
        &IdentStr::new("main").unwrap(),
        vec![],
        Vec::<Bytes>::new(),
        &mut gas_meter,
    );

    eprintln!("Move Runtime Cycle Count: {}", env::cycle_count() - start); // cycle count end - start
    env::commit(&result.unwrap().return_values);
}
