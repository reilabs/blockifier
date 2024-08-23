use std::collections::hash_map::{Entry, IntoIter};
use std::collections::{HashMap, HashSet};

use starknet_api::core::ClassHash;

use crate::state::state_api::StateReader;

pub trait VisitedPcsTrait
where
    Self: std::default::Default + IntoIterator,
{
    type Output;

    /// `pcs` type is matching the output from `runner.relocated_trace`
    fn insert(&mut self, class_hash: &ClassHash, pcs: &Vec<usize>);

    fn iter(&self) -> impl Iterator<Item = (&ClassHash, Vec<usize>)>;

    fn entry(&mut self, class_hash: ClassHash) -> Entry<'_, ClassHash, Self::Output>;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VisitedPcsSet(HashMap<ClassHash, HashSet<usize>>);
impl VisitedPcsSet {
    pub fn new() -> Self {
        VisitedPcsSet(HashMap::default())
    }
}
impl VisitedPcsTrait for VisitedPcsSet {
    type Output = HashSet<usize>;

    fn insert(&mut self, class_hash: &ClassHash, pcs: &Vec<usize>) {
        self.0.entry(*class_hash).or_default().extend(pcs);
    }

    fn iter(&self) -> impl Iterator<Item = (&ClassHash, Vec<usize>)> {
        self.0.iter().map(|(c, v)| (c, Vec::from_iter(v.clone())))
    }

    fn entry(&mut self, class_hash: ClassHash) -> Entry<'_, ClassHash, HashSet<usize>> {
        self.0.entry(class_hash)
    }
}
impl IntoIterator for VisitedPcsSet {
    type Item = (ClassHash, HashSet<usize>);
    type IntoIter = IntoIter<ClassHash, HashSet<usize>>;

    fn into_iter(self) -> IntoIter<ClassHash, HashSet<usize>> {
        self.0.into_iter()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VisitedPcsRaw(HashMap<ClassHash, Vec<Vec<usize>>>);
impl VisitedPcsRaw {
    pub fn new() -> Self {
        VisitedPcsRaw(HashMap::default())
    }
}
impl VisitedPcsTrait for VisitedPcsRaw {
    type Output = Vec<Vec<usize>>;

    fn insert(&mut self, class_hash: &ClassHash, pcs: &Vec<usize>) {
        self.0.entry(*class_hash).or_default().push(pcs.to_vec());
    }

    fn iter(&self) -> impl Iterator<Item = (&ClassHash, Vec<usize>)> {
        self.0.iter().flat_map(|(c, v)| v.iter().map(move |z| (c, z.clone())))
    }

    fn entry(
        &mut self,
        class_hash: ClassHash,
    ) -> Entry<'_, starknet_api::core::ClassHash, Vec<Vec<usize>>> {
        self.0.entry(class_hash)
    }
}
impl IntoIterator for VisitedPcsRaw {
    type Item = (ClassHash, Vec<Vec<usize>>);
    type IntoIter = IntoIter<ClassHash, Vec<Vec<usize>>>;

    fn into_iter(self) -> IntoIter<ClassHash, Vec<Vec<usize>>> {
        self.0.into_iter()
    }
}

#[derive(Debug, Default)]
pub struct CachedStateTest<S: StateReader> {
    pub state: S,
    /// A map from class hash to the set of PC values that were visited in the class.
    pub visited_pcs: VisitedPcsSet,
}
impl<S: StateReader> CachedStateTest<S> {
    pub fn new(state: S) -> Self {
        Self { state, visited_pcs: VisitedPcsSet::default() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::dict_state_reader::DictStateReader;

    fn get_state<V: VisitedPcsTrait>(_visited_pcs: &V) {
        todo!()
    }

    #[ignore]
    #[test]
    fn test_cached_state() {
        let state = DictStateReader::default();
        let _cached_state: CachedStateTest<DictStateReader> = CachedStateTest::new(state);
        get_state(&_cached_state.visited_pcs);
    }
}
