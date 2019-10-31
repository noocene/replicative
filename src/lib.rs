pub mod clock;
pub mod register;
use clock::Actor;
use std::{any::Any, fmt::Debug, marker::PhantomData};

pub struct Replicant<R: Replicative> {
    data: PhantomData<R>,
}

impl<R: Replicative> Replicant<R> {
    pub fn new(data: &'_ mut R, actor: u32) -> Self {
        data.prepare(Actor::new(actor), Handle { data: PhantomData });
        Replicant { data: PhantomData }
    }
}

pub struct Handle<R: ?Sized + Replicative> {
    data: PhantomData<R>,
}

impl<R: ?Sized + Replicative> Handle<R> {
    fn dispatch(&mut self, op: R::Op)
    where
        R::Op: Debug,
    {
        println!("{:?}", op);
    }
}

pub trait Replicative {
    type Op: Any;

    fn prepare(&mut self, this: Actor, handle: Handle<Self>);
    fn apply(&mut self, op: Self::Op);
}
