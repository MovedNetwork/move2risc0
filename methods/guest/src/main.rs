use bytes::Bytes;
use core::InMemoryStorage;
use move_core_types::account_address::AccountAddress;
use move_core_types::identifier::{IdentStr, Identifier};
use move_core_types::language_storage::ModuleId;
use move_vm_runtime::move_vm::MoveVM;
use move_vm_test_utils::gas_schedule::GasStatus;
use risc0_zkvm::guest::env;
use std::str::FromStr;

risc0_zkvm::guest::entry!(main);

fn main() {
    let start = env::cycle_count();

    // Read module storage from the host
    let storage: InMemoryStorage = env::read();

    let move_vm = MoveVM::new(vec![]).unwrap();
    let mut session = move_vm.new_session(&storage);
    let mut gas_meter = GasStatus::new_unmetered();

    // Run entry function on the addition module
    let module_id_add = ModuleId::new(AccountAddress::ZERO, Identifier::from_str("add").unwrap());
    let result_add = session.execute_entry_function(
        &module_id_add,
        &IdentStr::new("main").unwrap(),
        vec![],
        Vec::<Bytes>::new(),
        &mut gas_meter,
    );
    eprintln!("Arithmetic Result: {:?}", result_add);

    // Run entry function on the fibonacci module
    let module_id_fib = ModuleId::new(AccountAddress::ZERO, Identifier::from_str("fib").unwrap());
    let result_fib = session.execute_entry_function(
        &module_id_fib,
        &IdentStr::new("main").unwrap(),
        vec![],
        Vec::<Bytes>::new(),
        &mut gas_meter,
    );
    eprintln!("Fibonacci Result: {:?}", result_fib);

    eprintln!("Move Runtime Cycle Count: {}", env::cycle_count() - start); // cycle count end - start

    let mut output = result_add.unwrap().return_values;
    output.extend(result_fib.unwrap().return_values);
    env::commit(&output);
}
