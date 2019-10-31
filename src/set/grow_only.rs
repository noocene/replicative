use futures::{
    task::{Context, Poll},
    Stream,
};
use std::{
    fmt::{self, Debug, Formatter},
    ops::Deref,
    pin::Pin,
};
use void::Void;

use super::Set;

use crate::{
    cache::{Cache, Sequence},
    Handle, Replicative,
};

pub struct GrowOnly<T: Set + Clone + Unpin>
where
    <T as Set>::Item: Replicative + Clone + Unpin,
{
    data: T,
    handle: Sequence<Self>,
}

impl<T: Set + Clone + Unpin + Debug> Debug for GrowOnly<T>
where
    <T as Set>::Item: Replicative + Clone + Unpin + Debug,
{
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(
            f,
            "GrowOnly {{ data: {:?}, handle: {:?} }}",
            self.data, self.handle
        )
    }
}

impl<T: Set> Replicative for GrowOnly<T>
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
        let mut grow_only = Self::new();
        grow_only.merge(state)?;
        Ok(grow_only)
    }
}

impl<T: Set + Clone + Unpin> GrowOnly<T>
where
    <T as Set>::Item: Replicative + Clone + Unpin,
{
    pub fn new() -> Self {
        GrowOnly {
            data: T::new(),
            handle: Sequence::new(),
        }
    }
    pub fn insert(&mut self, item: <T as Set>::Item) -> bool {
        let item_is_new = self.data.insert(item.clone());
        if item_is_new {
            self.handle.dispatch(item)
        }
        item_is_new
    }
}

impl<T: Set + Clone + Unpin> Deref for GrowOnly<T>
where
    <T as Set>::Item: Replicative + Clone + Unpin,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T: Set + Clone + Unpin> Stream for GrowOnly<T>
where
    <T as Set>::Item: Replicative + Clone + Unpin,
    Self: Unpin,
{
    type Item = <T as Set>::Item;

    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context) -> Poll<Option<Self::Item>> {
        Poll::Ready(self.handle.next_cached())
    }
}
