use std::{
    collections::{BTreeSet, HashSet},
    hash::Hash,
};

pub mod grow_only;
pub use grow_only::GrowOnly;

pub trait Set: Extend<<Self as Set>::Item> + IntoIterator<Item = <Self as Set>::Item> {
    type Item;

    fn new() -> Self;
    fn insert(&mut self, value: <Self as Set>::Item) -> bool;
    fn contains(&self, value: &<Self as Set>::Item) -> bool;
    fn remove(&mut self, value: &<Self as Set>::Item) -> bool;
}

impl<T: Ord> Set for BTreeSet<T> {
    type Item = T;

    fn new() -> Self {
        BTreeSet::new()
    }
    fn insert(&mut self, value: <Self as Set>::Item) -> bool {
        self.insert(value)
    }
    fn contains(&self, value: &<Self as Set>::Item) -> bool {
        self.contains(value)
    }
    fn remove(&mut self, value: &<Self as Set>::Item) -> bool {
        self.remove(value)
    }
}

impl<T: Hash + Eq> Set for HashSet<T> {
    type Item = T;

    fn new() -> Self {
        HashSet::new()
    }
    fn insert(&mut self, value: <Self as Set>::Item) -> bool {
        self.insert(value)
    }
    fn contains(&self, value: &<Self as Set>::Item) -> bool {
        self.contains(value)
    }
    fn remove(&mut self, value: &<Self as Set>::Item) -> bool {
        self.remove(value)
    }
}
