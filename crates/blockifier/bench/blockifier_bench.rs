//! Benchmark module for the blockifier crate. It provides functionalities to benchmark
//! various aspects related to transferring between accounts, including preparation
//! and execution of transfers.
//!
//! The main benchmark function is `transfers_benchmark`, which measures the performance
//! of transfers between randomly created accounts, which are iterated over round-robin.
//!
//! The other benchmark function is `execution_benchmark` which measures the performance of the
//! method [`blockifier::transactions::transaction::ExecutableTransaction::execute`] by executing
//! the entry point `advance_counter` of the test contract.
//!
//! //! Run the benchmarks using `cargo bench --bench blockifier_bench`.

use blockifier::context::BlockContext;
use blockifier::invoke_tx_args;
use blockifier::state::cached_state::CachedState;
use blockifier::test_utils::contracts::FeatureContract;
use blockifier::test_utils::dict_state_reader::DictStateReader;
use blockifier::test_utils::initial_test_state::test_state;
use blockifier::test_utils::transfers_generator::{
    RecipientGeneratorType, TransfersGenerator, TransfersGeneratorConfig,
};
use blockifier::test_utils::{create_calldata, CairoVersion, NonceManager, BALANCE};
use blockifier::transaction::account_transaction::AccountTransaction;
use blockifier::transaction::test_utils::{account_invoke_tx, block_context, max_resource_bounds};
use blockifier::transaction::transactions::ExecutableTransaction;
use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use starknet_api::felt;

pub fn transfers_benchmark(c: &mut Criterion) {
    let transfers_generator_config = TransfersGeneratorConfig {
        recipient_generator_type: RecipientGeneratorType::Random,
        ..Default::default()
    };
    let mut transfers_generator = TransfersGenerator::new(transfers_generator_config);
    // Create a benchmark group called "transfers", which iterates over the accounts round-robin
    // and performs transfers.
    c.bench_function("transfers", |benchmark| {
        benchmark.iter(|| {
            transfers_generator.execute_transfers();
        })
    });
}

pub fn execution_benchmark(c: &mut Criterion) {
    /// This function sets up and returns all the objects required to execute an invoke transaction.
    fn prepare_account_tx() -> (AccountTransaction, CachedState<DictStateReader>, BlockContext) {
        let block_context = block_context();
        let max_resource_bounds = max_resource_bounds();
        let cairo_version = CairoVersion::Cairo1;
        let account = FeatureContract::AccountWithoutValidations(cairo_version);
        let test_contract = FeatureContract::TestContract(cairo_version);
        let state =
            test_state(block_context.chain_info(), BALANCE, &[(account, 1), (test_contract, 1)]);
        let account_address = account.get_instance_address(0);
        let contract_address = test_contract.get_instance_address(0);
        let index = felt!(123_u32);
        let base_tx_args = invoke_tx_args! {
            resource_bounds: max_resource_bounds,
            sender_address: account_address,
        };

        let mut nonce_manager = NonceManager::default();
        let counter_diffs = [101_u32, 102_u32];
        let initial_counters = [felt!(counter_diffs[0]), felt!(counter_diffs[1])];
        let calldata_args = vec![index, initial_counters[0], initial_counters[1]];

        let account_tx = account_invoke_tx(invoke_tx_args! {
            nonce: nonce_manager.next(account_address),
            calldata:
                create_calldata(contract_address, "advance_counter", &calldata_args),
            ..base_tx_args
        });
        (account_tx, state, block_context)
    }
    c.bench_function("execution", move |benchmark| {
        benchmark.iter_batched(
            prepare_account_tx,
            |(account_tx, mut state, block_context)| {
                account_tx.execute(&mut state, &block_context, true, true).unwrap()
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, transfers_benchmark, execution_benchmark);
criterion_main!(benches);
