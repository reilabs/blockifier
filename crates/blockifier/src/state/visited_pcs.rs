use std::collections::hash_map::{Entry, IntoIter, Iter};
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
    type T: Clone;

    fn new() -> Self;

    /// The function `insert` reads the program counters returned by the cairo vm trace.
    ///
    /// The elements of the vector `pcs` match the type of field `pc` in
    /// [`cairo_vm::vm::trace::trace_entry::RelocatedTraceEntry`]
    fn insert(&mut self, class_hash: &ClassHash, pcs: &Vec<usize>);

    /// The function `extend` is used to extend an instance of `VisitedPcs` with another one.
    fn extend(&mut self, class_hash: &ClassHash, pcs: &Self::T);

    /// This function returns an iterator of `VisitedPcs`.
    fn iter(&self) -> impl Iterator<Item = (&ClassHash, &Self::T)>;

    /// Get the recorded visited program counters for a specific `class_hash`.
    fn entry(&mut self, class_hash: ClassHash) -> Entry<'_, ClassHash, Self::T>;

    /// Marks the given PC values as visited for the given class hash.
    fn add_visited_pcs(state: &mut dyn State, class_hash: &ClassHash, pcs: Self::T);

    /// This function returns the program counters in a set.
    fn to_set(pcs: Self::T) -> HashSet<usize>;
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct VisitedPcsSet(HashMap<ClassHash, HashSet<usize>>);
impl VisitedPcsSet {
    pub fn iter(&self) -> impl Iterator<Item = (&ClassHash, &HashSet<usize>)> {
        self.into_iter()
    }
}
impl VisitedPcs for VisitedPcsSet {
    type T = HashSet<usize>;

    fn new() -> Self {
        VisitedPcsSet(HashMap::default())
    }

    fn insert(&mut self, class_hash: &ClassHash, pcs: &Vec<usize>) {
        self.0.entry(*class_hash).or_default().extend(pcs);
    }

    fn iter(&self) -> impl Iterator<Item = (&ClassHash, &Self::T)> {
        self.0.iter()
    }

    fn entry(&mut self, class_hash: ClassHash) -> Entry<'_, ClassHash, HashSet<usize>> {
        self.0.entry(class_hash)
    }

    fn add_visited_pcs(state: &mut dyn State, class_hash: &ClassHash, pcs: Self::T) {
        state.add_visited_pcs(*class_hash, &Vec::from_iter(pcs));
    }

    fn extend(&mut self, class_hash: &ClassHash, pcs: &Self::T) {
        self.0.entry(*class_hash).or_default().extend(pcs);
    }

    fn to_set(pcs: Self::T) -> HashSet<usize> {
        pcs
    }
}
impl IntoIterator for VisitedPcsSet {
    type Item = (ClassHash, HashSet<usize>);
    type IntoIter = IntoIter<ClassHash, HashSet<usize>>;

    fn into_iter(self) -> IntoIter<ClassHash, HashSet<usize>> {
        self.0.into_iter()
    }
}
impl<'a> IntoIterator for &'a VisitedPcsSet {
    type Item = (&'a ClassHash, &'a HashSet<usize>);
    type IntoIter = Iter<'a, ClassHash, HashSet<usize>>;

    fn into_iter(self) -> Iter<'a, ClassHash, HashSet<usize>> {
        self.0.iter()
    }
}
