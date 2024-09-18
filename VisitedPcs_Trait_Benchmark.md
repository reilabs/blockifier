# Adding the `VisitedPcs` Trait

The state of the blockifier as of commit
`14e6a87722c1d0c757b1aa2756ffabe3f248fd7d` doesn't store the complete vector of
visited program counters for each entry-point in an invoke transaction. Instead,
visited program counters are pushed into a `HashSet`. Unfortunately this limits
the ability to perform profiling operations, as many require a record of the
full trace returned from the `cairo-vm`.

In order to enable more in-depth tracing use-cases, we have introduced the
`VisitedPcs` trait which allows the user to process the visited program counters
as they see fit.

## Before Changes

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

This snipped has been extracted from commit
[14e6a87722c1d0c757b1aa2756ffabe3f248fd7d](https://github.com/reilabs/blockifier/blob/14e6a87722c1d0c757b1aa2756ffabe3f248fd7d/crates/blockifier/src/state/cached_state.rs#L36)

## After Changes

> [!NOTE]
> The new code is developed in the branch `visited_pcs_trait` and the
> current head of the branch is at commit
> [`bdb1b49331aad91d445ac2155baa40fa783bcf7f`](https://github.com/reilabs/blockifier/blob/visited_pcs_trait/crates/blockifier/src/state/cached_state.rs#L37).
> This will change once these changes are merged in the main branch.

`VisitedPcs` is added as an additional generic parameter of `CachedState`.

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
the name `VisitedPcsSet`. This mimics the existing `HashSet<usize>` usage of
this field. For test purposes, `CachedState` is instantiated using
`VisitedPcsSet`.

## Performance Considerations

Given the importance of the blockifier's performance in the Starknet ecosystem,
measuring the impact of adding the aforementioned `VisitedPcs` trait is very
important. The existing bechmark `transfers` doesn't cover operations that use
the `CachedState`, and therefore we have designed new ones as follows:

- `cached_state`: this benchmark tests the performance impact of populating
  `visited_pcs` (implemented using `VisitedPcsSet`) with a realistic amount of
  visited program counters. The size of the sets is taken from transaction
  `0x0177C9365875CAA840EA8F03F97B0E3A8EE8851A8B952BF157B5DBD4FECCB060` on
  mainnet. This transaction has been chosen randomly but there is no assurance
  that it's representative of the most common Starknet invoke transaction. This
  benchmark tests the write performance of visited program counters in the state
  struct.
- `execution`: this benchmark simulates a whole invoke transaction using a dummy
  contract.

## Performance Impact

The `bench.sh` script has been added to benchmark the performance impact of
these changes.

The benchmark results presented below were conducted under the following
conditions:

- **Operating System:** Debian 12 (Bookworm) running in a VMWare Workstation 17
  VM on Windows 10 22H2
- **Hardware:** i9-9900K @ 5.0 GHz, 64GB of RAM, Samsung 990 Pro NVMe SSD.
- **Rust Toolchain:** 1.78-x86_64-unknown-linux-gnu / rust 1.78.0 (9b00956e5
  2024-04-29).

The script was called as follows, but you may need to [adjust the commit
hashes](#after-changes) in question to reproduce these results:

`bash scripts/bench.sh 14e6a87722c1d0c757b1aa2756ffabe3f248fd7d e39ae0be4cec31938399199e0a1070279b4a78ed`

The noise threshold and confidence intervals are kept as per default
Criterion.rs configuration.

The results are as follows:

| Benchmark    | Time (ms) | Time change (%) | Criterion.rs report           |
| ------------ | --------- | --------------- | ----------------------------- |
| transfers    | 94.448    | +0.1080         | No change in performance      |
| execution    | 1.2882    | -1.7216         | Change within noise threshold |
| cached_state | 5.2330    | -0.8703         | No change in performance      |

Criterion's inbuilt confidence analysis suggests that these results have no
statistical significant and do not represent real-world performance changes.
