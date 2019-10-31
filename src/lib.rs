pub mod clock;

use clock::Actor;

use void::Void;

use std::{
    collections::{BTreeSet, HashSet},
    fmt::{self, Debug, Display, Formatter},
    hash::Hash,
    ops::Deref,
    pin::Pin,
};

use futures::{task::Context, Poll, Stream};

use failure::Fail;

#[derive(Clone, Copy, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
pub struct Object(u32);

#[derive(Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
pub struct Reference(Actor, Object);

pub trait Handle<R: Replicative>: Send {
    fn dispatch(&mut self, op: R::Op);
    fn this(&self) -> Reference;
}

impl<R: Replicative> Debug for Box<dyn Handle<R>> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "handle at reference {:?}", self.this())
    }
}

pub trait Replicative: Sized + Stream<Item = <Self as Replicative>::Op> {
    type Op;
    type State;
    type MergeError: Fail;

    fn apply(&mut self, op: Self::Op);
    fn prepare<H: Handle<Self> + 'static>(&mut self, handle: H);
    fn new(state: Self::State) -> Result<Self, Self::MergeError>;
    fn merge(&mut self, state: Self::State) -> Result<(), Self::MergeError>;
    fn fetch(&self) -> Self::State;
}

#[derive(Clone, Hash, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct Leaf<T>(T);

impl<T> Leaf<T> {
    pub fn new(data: T) -> Self {
        Leaf(data)
    }
}

impl<T> Deref for Leaf<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Fail, Debug)]
pub struct InvalidMergeError;

impl Display for InvalidMergeError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "cannot mutate leaf")
    }
}

impl<T: Clone> Replicative for Leaf<T> {
    type Op = Void;
    type MergeError = InvalidMergeError;
    type State = T;

    fn apply(&mut self, _: Self::Op) {}
    fn prepare<H: Handle<Self> + 'static>(&mut self, _: H) {}
    fn new(data: Self::State) -> Result<Self, Self::MergeError> {
        Ok(Leaf(data))
    }
    fn merge(&mut self, _: Self::State) -> Result<(), Self::MergeError> {
        Err(InvalidMergeError)
    }
    fn fetch(&self) -> Self::State {
        self.0.clone()
    }
}

impl<T> Stream for Leaf<T> {
    type Item = Void;

    fn poll_next(self: Pin<&mut Self>, _: &mut Context) -> Poll<Option<Self::Item>> {
        Poll::Ready(None)
    }
}

#[derive(Debug)]
pub enum SeqCache<T: Replicative> {
    Cache(Vec<T::Op>),
    Handle(Box<dyn Handle<T>>),
}

impl<T: Replicative> SeqCache<T>
where
    T::Op: Clone,
{
    fn new() -> Self {
        SeqCache::Cache(Vec::new())
    }
    fn prepare<H: Handle<T> + 'static>(&mut self, mut handle: H) {
        use SeqCache::{Cache, Handle};
        if let Cache(items) = self {
            for item in items {
                handle.dispatch(item.clone());
            }
        }
        *self = Handle(Box::new(handle))
    }
    fn dispatch(&mut self, op: T::Op) {
        use SeqCache::{Cache, Handle};
        match self {
            Cache(items) => items.push(op),
            Handle(handle) => handle.dispatch(op),
        }
    }
    fn next_cached(&mut self) -> Option<T::Op> {
        use SeqCache::Cache;
        if let Cache(items) = self {
            return items.pop();
        }
        None
    }
}

pub struct Grow<T: Set + Clone + Unpin>
where
    <T as Set>::Item: Replicative + Clone + Unpin,
{
    data: T,
    handle: SeqCache<Self>,
}

impl<T: Set + Clone + Unpin + Debug> Debug for Grow<T>
where
    <T as Set>::Item: Replicative + Clone + Unpin + Debug,
{
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(
            f,
            "Grow {{ data: {:?}, handle: {:?} }}",
            self.data, self.handle
        )
    }
}

impl<T: Set> Replicative for Grow<T>
where
    T: Clone + Unpin,
    <T as Set>::Item: Replicative + Clone + Unpin,
{
    type Op = <T as Set>::Item;
    type State = T;
    type MergeError = Void;

    fn merge(&mut self, state: Self::State) -> Result<(), Self::MergeError> {
        self.data.extend(state);
        Ok(())
    }
    fn apply(&mut self, op: Self::Op) {
        self.data.insert(op);
    }
    fn prepare<H: Handle<Self> + 'static>(&mut self, handle: H) {
        self.handle.prepare(handle)
    }
    fn fetch(&self) -> Self::State {
        self.data.clone()
    }
    fn new(state: Self::State) -> Result<Self, Self::MergeError> {
        let mut grow = Self::new();
        grow.merge(state)?;
        Ok(grow)
    }
}

impl<T: Set + Clone + Unpin> Grow<T>
where
    <T as Set>::Item: Replicative + Clone + Unpin,
{
    pub fn new() -> Self {
        Grow {
            data: T::new(),
            handle: SeqCache::new(),
        }
    }
    pub fn insert(&mut self, item: <T as Set>::Item) {
        if self.data.insert(item.clone()) {
            self.handle.dispatch(item)
        }
    }
}

impl<T: Set + Clone + Unpin> Deref for Grow<T>
where
    <T as Set>::Item: Replicative + Clone + Unpin,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T: Set + Clone + Unpin> Stream for Grow<T>
where
    <T as Set>::Item: Replicative + Clone + Unpin,
    Self: Unpin,
{
    type Item = <T as Set>::Item;

    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context) -> Poll<Option<Self::Item>> {
        Poll::Ready(self.handle.next_cached())
    }
}

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
