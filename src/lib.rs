pub mod cache;
pub mod clock;
pub mod counter;
pub mod leaf;
pub mod set;
pub use leaf::Leaf;

use clock::Actor;

use failure::Fail;
use futures::Stream;
use std::{
    fmt::{self, Debug, Formatter},
    hash::Hash,
};

#[derive(Clone, Copy, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
pub struct Object(u32);

#[derive(Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
pub struct Reference(Actor, Object);

impl Reference {
    fn actor(&self) -> Actor {
        self.0
    }
}

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
    type Target: Replicative;
    type ApplyError: Fail;

    fn apply(&mut self, origin: Actor, op: Self::Op) -> Result<(), Self::ApplyError>;
    fn prepare<H: Handle<Self::Target> + 'static>(&mut self, handle: H);
    fn new(state: Self::State) -> Result<Self, Self::MergeError>;
    fn merge(&mut self, state: Self::State) -> Result<(), Self::MergeError>;
    fn fetch(&self) -> Self::State;
}
