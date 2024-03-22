use move_core_types::effects::ChangeSet;
use move_vm_runtime::session::SerializedReturnValues;
use core::{EntryFunction, SmtStorage, Transaction, TransactionPayload};
use move_vm_runtime::move_vm::MoveVM;
use move_vm_test_utils::gas_schedule::GasStatus;
use move_binary_format::errors::VMError;
use risc0_smt::Smt;
use risc0_zkvm::{guest::{env, sha}, sha::Sha256};

risc0_zkvm::guest::entry!(main);

/// Executes a transaction against a given state root.
/// Produces the change set and transaction output.
fn main() {
    let start = env::cycle_count();

    // Read transaction from the host
    let transaction: Transaction = env::read();

    // Read the SMT from the host
    let smt: Smt = env::read();

    // Log the state root being used.
    env::commit(smt.get_root());

    let outcome = match transaction.payload {
        TransactionPayload::EntryFunction(entry_fn) => execute_entry_function(entry_fn, &smt),
    };

    // TODO error handling
    let (output, change_set) = outcome.unwrap();

    let output_bytes = output.return_values.into_iter().flat_map(|x| x.0);
    let change_bytes = core::codec::serialize_changes(&change_set);

    let all_bytes: Vec<u8> = output_bytes.chain(change_bytes).collect();
    let hash = sha::Impl::hash_bytes(&all_bytes);

    // Log the hash of the output
    env::commit(&hash);

    // Return the actual value to the host
    env::write_slice(&all_bytes);

    eprintln!("Move Runtime Cycle Count: {}", env::cycle_count() - start); // cycle count end - start
}

fn execute_entry_function(entry_fn: EntryFunction, smt: &Smt) -> Result<(SerializedReturnValues, ChangeSet), VMError> {
    let move_vm = MoveVM::new(vec![]).unwrap();
    let storage = SmtStorage::new(smt);
    let mut session = move_vm.new_session(&storage);
    let mut gas_meter = GasStatus::new_unmetered();

    let result = session.execute_entry_function(&entry_fn.module, &entry_fn.function, entry_fn.ty_args, entry_fn.args, &mut gas_meter)?;
    let change_set = session.finish()?;
    Ok((result, change_set))
}
