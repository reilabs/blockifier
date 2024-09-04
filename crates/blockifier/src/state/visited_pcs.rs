use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;

use starknet_api::core::ClassHash;

use super::state_api::State;

/// This trait is used in `CachedState` to record visited pcs of an entry point call.
pub trait VisitedPcs
where
    Self: Default + Debug,
{
    /// This is the type which contains visited program counters.
    type Pcs: Clone;

    fn new() -> Self;

    /// The function `insert` reads the program counters returned by the cairo vm trace.
    ///
    /// The elements of the vector `pcs` match the type of field `pc` in
    /// [`cairo_vm::vm::trace::trace_entry::RelocatedTraceEntry`]
    fn insert(&mut self, class_hash: &ClassHash, pcs: &[usize]);

    /// The function `extend` is used to extend an instance of `VisitedPcs` with another one.
    fn extend(&mut self, class_hash: &ClassHash, pcs: &Self::Pcs);

    /// This function returns an iterator of `VisitedPcs`.
    fn iter(&self) -> impl Iterator<Item = (&ClassHash, &Self::Pcs)>;

    /// Get the recorded visited program counters for a specific `class_hash`.
    fn entry(&mut self, class_hash: ClassHash) -> Entry<'_, ClassHash, Self::Pcs>;

    /// Marks the given PC values as visited for the given class hash.
    fn add_visited_pcs(state: &mut dyn State, class_hash: &ClassHash, pcs: Self::Pcs);

    /// This function returns the program counters in a set.
    fn to_set(pcs: Self::Pcs) -> HashSet<usize>;
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct VisitedPcsSet(HashMap<ClassHash, HashSet<usize>>);
impl VisitedPcs for VisitedPcsSet {
    type Pcs = HashSet<usize>;

    fn new() -> Self {
        VisitedPcsSet(HashMap::default())
    }

    fn insert(&mut self, class_hash: &ClassHash, pcs: &[usize]) {
        self.0.entry(*class_hash).or_default().extend(pcs);
    }

    fn iter(&self) -> impl Iterator<Item = (&ClassHash, &Self::Pcs)> {
        self.0.iter()
    }

    fn entry(&mut self, class_hash: ClassHash) -> Entry<'_, ClassHash, HashSet<usize>> {
        self.0.entry(class_hash)
    }

    fn add_visited_pcs(state: &mut dyn State, class_hash: &ClassHash, pcs: Self::Pcs) {
        state.add_visited_pcs(*class_hash, &Vec::from_iter(pcs));
    }

    fn extend(&mut self, class_hash: &ClassHash, pcs: &Self::Pcs) {
        self.0.entry(*class_hash).or_default().extend(pcs);
    }

    fn to_set(pcs: Self::Pcs) -> HashSet<usize> {
        pcs
    }
}
