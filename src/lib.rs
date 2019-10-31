pub mod clock;
pub mod register;
use clock::Actor;
use std::{
    any::Any,
    fmt::Debug,
    marker::PhantomData,
    pin::Pin,
    collections::HashMap,
    task::{Context, Poll},
};

use futures::{
    channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender,SendError},
    executor::ThreadPool,
    SinkExt, Stream, Sink,StreamExt
};

pub struct Replicant<R: Replicative> {
    data: PhantomData<R>,
    last: Reference,
    in_actions: HashMap<Reference, Pin<Box<UnboundedSender<Action>>>>,
    out_actions: (Pin<Box<UnboundedReceiver<Action>>>, UnboundedSender<Action>),
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
    data: Box<dyn Any + Send>,
}

impl<R: Replicative + Send + 'static> Replicant<R> {
    pub fn new(data: &'_ mut R, actor: u32) -> Self {
        let (sender, receiver) = unbounded();
        let (isender, mut ireceiver): (_, UnboundedReceiver<Action>) = unbounded();
        let mut r = data.clone();
        ThreadPool::new().unwrap().spawn_ok(async move {
            while let Some(item) = ireceiver.next().await {
                r.apply(*item.data.downcast().unwrap());
            }
        });
        let this = Actor::new(actor);
        let mut in_actions = HashMap::new();
        let reference = Reference::new(this);
        in_actions.insert(reference.clone(), Box::pin(isender));
        data.prepare(
            this,
            Handle {
                data: PhantomData,
                actions: sender.clone(),
                reference: reference.clone(),
            },
        );
        Replicant {
            data: PhantomData,
            out_actions: (Box::pin(receiver), sender),
            last: reference,
            in_actions
        }
    }
}

impl<R: Replicative + Unpin> Stream for Replicant<R> {
    type Item = Action;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        self.out_actions.0.as_mut().poll_next(cx)
    }
}

impl<R: Replicative + Unpin> Sink<Action> for Replicant<R> {
    type Error = SendError;

    fn start_send(mut self: Pin<&mut Self>, item: Action) -> Result<(), Self::Error> {
        if let Some(channel) = self.in_actions.get_mut(&item.target) {
            channel.as_mut().start_send(item)
        } else {
            panic!("no channel")
        }
    }
    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        if let Some(result) = self
            .in_actions
            .values_mut()
            .map(|item| item.as_mut().poll_ready(cx))
            .find(|poll| match poll {
                Poll::Ready(_) => false,
                _ => true,
            })
        {
            result
        } else {
            Poll::Ready(Ok(()))
        }
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        if let Some(result) = self
            .in_actions
            .values_mut()
            .map(|item| item.as_mut().poll_flush(cx))
            .find(|poll| match poll {
                Poll::Ready(_) => false,
                _ => true,
            })
        {
            result
        } else {
            Poll::Ready(Ok(()))
        }
    }
    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        if let Some(result) = self
            .in_actions
            .values_mut()
            .map(|item| item.as_mut().poll_close(cx))
            .find(|poll| match poll {
                Poll::Ready(_) => false,
                _ => true,
            })
        {
            result
        } else {
            Poll::Ready(Ok(()))
        }
    }
}

pub struct Handle<R: ?Sized + Replicative> {
    data: PhantomData<R>,
    actions: UnboundedSender<Action>,
    reference: Reference,
}

impl<R: Send + ?Sized + Replicative> Handle<R> {
    fn dispatch(&mut self, op: R::Op)
    where
        R::Op: Debug + Send,
    {
        let mut actions = self.actions.clone();
        let reference = self.reference.clone();
        ThreadPool::new().unwrap().spawn_ok(async move {
            let _ = actions
                .send(Action {
                    target: reference,
                    data: Box::new(op),
                })
                .await;
        })
    }
}

pub trait Replicative: Clone {
    type Op: Any;

    fn prepare(&mut self, this: Actor, handle: Handle<Self>);
    fn apply(&mut self, op: Self::Op);
}
