use failure::Fail;
use futures::{
    task::{Context, Poll},
    Stream,
};
use std::{
    fmt::{self, Display, Formatter},
    ops::Deref,
    pin::Pin,
};
use void::Void;

use crate::{clock::Actor, Handle, Replicative};

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
pub struct MergeError;

impl Display for MergeError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "cannot mutate leaf")
    }
}

impl<T: Clone> Replicative for Leaf<T> {
    type Op = Void;
    type Target = Self;
    type MergeError = MergeError;
    type ApplyError = Void;
    type State = T;

    fn apply(&mut self, _: Actor, _: Self::Op) -> Result<(), Self::ApplyError> {
        Ok(())
    }
    fn prepare<H: Handle<Self> + 'static>(&mut self, _: H) {}
    fn new(data: Self::State) -> Result<Self, Self::MergeError> {
        Ok(Leaf(data))
    }
    fn merge(&mut self, _: Self::State) -> Result<(), Self::MergeError> {
        Err(MergeError)
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
