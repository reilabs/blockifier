# Addition of `VisitedPcs` trait

Current blockifier doesn't store the complete vector of visited program counters
for each entry point call in an invoke transaction. Instead, visited program
counters are pushed in a `HashSet`. This is a limiting factor to perform
profiling operations which require record of the full trace returned by
`cairo-vm`. More flexibility is added to the blockifier with the introduction of
trait `VisitedPcs` which allows the user to process visited program counters in
the most suitable way for the task.

## Existing code

Visited program counters are kept in the `CachedState` structure as shown below:

```rust
#[derive(Debug)]
pub struct CachedState<S: StateReader> {
    pub state: S,
    // Invariant: read/write access is managed by CachedState.
    // Using interior mutability to update caches during `State`'s immutable getters.
    pub(crate) cache: RefCell<StateCache>,
    pub(crate) class_hash_to_class: RefCell<ContractClassMapping>,
    /// A map from class hash to the set of PC values that were visited in the class.
    pub visited_pcs: HashMap<ClassHash, HashSet<usize>>,
}
```

## New code

`VisitedPcs` is an additional generic parameter of `CachedState`.

```rust
#[derive(Debug)]
pub struct CachedState<S: StateReader, V: VisitedPcs> {
    pub state: S,
    // Invariant: read/write access is managed by CachedState.
    // Using interior mutability to update caches during `State`'s immutable getters.
    pub(crate) cache: RefCell<StateCache>,
    pub(crate) class_hash_to_class: RefCell<ContractClassMapping>,
    /// A map from class hash to the set of PC values that were visited in the class.
    pub visited_pcs: V,
}
```

An implementation of the trait `VisitedPcs` is included in the blockifier with
the name `VisitedPcsSet` and it mimics the existing `HashSet<usize>`. Also, for
test purposes, `CachedState` is instantiated using `VisitedPcsSet`.

## Performance considerations

Given the importance of the blockifier in the Starknet ecosystem, we want to
measure the performance impact of adding the trait `VisitedPcs`. The existing
bechmark `transfers` doesn't cover operations with `CachedState` therefore we
need to design new ones. We have created two new benchmarks:

- `cached_state`: this benchmark tests the performance impact of populating
  `visited_pcs` (implemented using `VisitedPcsSet`) with a realistic amount of
  visited program counters. The size of the sets is taken from transaction
  `0x0177C9365875CAA840EA8F03F97B0E3A8EE8851A8B952BF157B5DBD4FECCB060` in the
  mainnet. This transaction has been chosen randomly, but there is no assurance
  that it's representative of the most common Starknet invoke transaction. This
  benchmark tests the write performance of visited program counters in the state
  struct.
- `execution`: this benchmark simulates a whole invoke transaction using a dummy
  contract.

## Performance impact

A script `bench.sh` has been added to benchmark the performance impact of these
changes: it is called as
`bash scripts/bench.sh 14e6a87722c1d0c757b1aa2756ffabe3f248fd7d e39ae0be4cec31938399199e0a1070279b4a78ed`.
The computer running the benchmark is: Debian VM over Windows 10 with VMWare
Workstation 17, i9-9900K, 64GB RAM, Samsung 990 Pro NVME SSD.

The Rust toolchain used is:
```
1.78-x86_64-unknown-linux-gnu (default)
rustc 1.78.0 (9b00956e5 2024-04-29)
```

Noise threshold and confidence intervals are kept as per default Criterion.rs
configuration.

The results are shown in the following table:

| Benchmark    | Time (ms) | Time change (%) | Criterion.rs report           |
| ------------ | --------- | --------------- | ----------------------------- |
| transfers    | 94.448    | +0.1080         | No change in performance      |
| execution    | 1.2882    | -1.7216         | Change within noise threshold |
| cached_state | 5.2330    | -0.8703         | No change in performance      |

The analysis of Criterion.rs determines that there isn't statistically
significant performance decrese.
