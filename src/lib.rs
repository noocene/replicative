pub mod clock;
pub mod register;
use clock::Actor;
use std::{
    any::Any,
    fmt::Debug,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use futures::{
    channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender},
    executor::ThreadPool,
    SinkExt, Stream,
};

pub struct Replicant<R: Replicative> {
    data: PhantomData<R>,
    last: Reference,
    actions: (Pin<Box<UnboundedReceiver<Action>>>, UnboundedSender<Action>),
}

#[derive(Clone, Copy, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
#[repr(transparent)]
pub struct Object(u32);

impl Object {
    fn next(&self) -> Self {
        Object(self.0 + 1)
    }
    fn on(self, actor: Actor) -> Reference {
        Reference(actor, self)
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
pub struct Reference(Actor, Object);

impl Reference {
    fn new(actor: Actor) -> Self {
        Reference(actor, Object(1))
    }
    fn next(&self) -> Self {
        Reference(self.0, self.1.next())
    }
}

#[derive(Debug)]
pub struct Action {
    target: Reference,
    origin: Actor,
    data: Box<dyn Any + Send>,
}

impl<R: Replicative> Replicant<R> {
    pub fn new(data: &'_ mut R, actor: u32) -> Self {
        let (sender, receiver) = unbounded();
        let this = Actor::new(actor);
        let reference = Reference::new(this);
        data.prepare(
            this,
            Handle {
                data: PhantomData,
                actions: sender.clone(),
                origin: this,
                reference: reference.clone(),
            },
        );
        Replicant {
            data: PhantomData,
            actions: (Box::pin(receiver), sender),
            last: reference,
        }
    }
}

impl<R: Replicative + Unpin> Stream for Replicant<R> {
    type Item = Action;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        self.actions.0.as_mut().poll_next(cx)
    }
}

pub struct Handle<R: ?Sized + Replicative> {
    data: PhantomData<R>,
    actions: UnboundedSender<Action>,
    reference: Reference,
    origin: Actor,
}

impl<R: Send + ?Sized + Replicative> Handle<R> {
    fn dispatch(&mut self, op: R::Op)
    where
        R::Op: Debug + Send,
    {
        let mut actions = self.actions.clone();
        let reference = self.reference.clone();
        let origin = self.origin.clone();
        ThreadPool::new().unwrap().spawn_ok(async move {
            let _ = actions
                .send(Action {
                    target: reference,
                    data: Box::new(op),
                    origin,
                })
                .await;
        })
    }
}

pub trait Replicative {
    type Op: Any;

    fn prepare(&mut self, this: Actor, handle: Handle<Self>);
    fn apply(&mut self, op: Self::Op);
}
