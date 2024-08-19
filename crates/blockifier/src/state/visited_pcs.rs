use std::collections::hash_map::{Entry, IntoIter, Iter};
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;

use starknet_api::core::ClassHash;

use crate::state::state_api::StateReader;

// pub trait VisitedPcsTraitSecond
// where
//     Self: std::default::Default + IntoIterator + FromIterator,
// {
//     type Collection;

//     /// `pcs` type is matching the output from `runner.relocated_trace`
//     fn insert(&mut self, class_hash: &ClassHash, pcs: &Vec<usize>);

//     fn iter(&self) -> Iter<'_, ClassHash, Self::Collection>;

//     fn iter_second(&self) -> impl Iterator<Item = Self::Collection>;

//     fn entry(&mut self, class_hash: ClassHash) -> Entry<'_, ClassHash, Self::Collection>;
// }

pub trait VisitedPcsTrait<T>
where
    Self: std::default::Default + Sized,
    T: Sized,
{
    /// `pcs` type is matching the output from `runner.relocated_trace`
    fn insert(&mut self, class_hash: &ClassHash, pcs: &Vec<usize>);

    fn iter(&self) -> Iter<'_, ClassHash, T>;

    fn entry(&mut self, class_hash: ClassHash) -> Entry<'_, ClassHash, T>;
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct VisitedPcsSet(HashMap<ClassHash, HashSet<usize>>);
impl VisitedPcsSet {
    pub fn new() -> Self {
        VisitedPcsSet(HashMap::default())
    }
}
impl VisitedPcsTrait<HashSet<usize>> for VisitedPcsSet {
    fn insert(&mut self, class_hash: &ClassHash, pcs: &Vec<usize>) {
        self.0.entry(*class_hash).or_default().extend(pcs);
    }

    fn iter(&self) -> Iter<'_, ClassHash, HashSet<usize>> {
        self.0.iter()
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
impl<'a> IntoIterator for &'a VisitedPcsSet {
    type Item = (&'a ClassHash, &'a HashSet<usize>);
    type IntoIter = Iter<'a, ClassHash, HashSet<usize>>;

    fn into_iter(self) -> Iter<'a, ClassHash, HashSet<usize>> {
        self.0.iter()
    }
}

#[derive(Debug, Default)]
pub struct CachedStateTest<T, S: StateReader, V: VisitedPcsTrait<T>> {
    pub state: S,
    /// A map from class hash to the set of PC values that were visited in the class.
    pub visited_pcs: V,
    _marker: PhantomData<T>,
}
impl<T, S: StateReader, V: VisitedPcsTrait<T>> CachedStateTest<T, S, V> {
    pub fn new(state: S) -> Self {
        Self { state, visited_pcs: V::default(), _marker: PhantomData }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::dict_state_reader::DictStateReader;

    #[test]
    fn test_cached_state() {
        let state = DictStateReader::default();
        let _cached_state: CachedStateTest<HashSet<usize>, DictStateReader, VisitedPcsSet> =
            CachedStateTest::new(state);
    }
}
