pub mod clock;
pub mod register;
use clock::Actor;
use std::{
    any::Any,
    fmt::Debug,
    marker::PhantomData,
    pin::Pin,
    sync::{Arc, Mutex},
    collections::HashMap,
    task::{Context, Poll},
};

use futures::{
    channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender},
    executor::ThreadPool,
    SinkExt, Stream, Sink
};

pub struct Replicant<R: Replicative> {
    data: PhantomData<R>,
    last: Reference,
    in_actions: Arc<Mutex<HashMap<Reference, Pin<Box<dyn Sink<Action, Error = ()> + Send>>>>>,
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
                reference: reference.clone(),
            },
        );
        Replicant {
            data: PhantomData,
            out_actions: (Box::pin(receiver), sender),
            last: reference,
            in_actions: Arc::new(Mutex::new(HashMap::new()))
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
    type Error = ();

    fn start_send(self: Pin<&mut Self>, item: Action) -> Result<(), Self::Error> {
        if let Some(channel) = self.in_actions.lock().unwrap().get_mut(&item.target) {
            channel.as_mut().start_send(item)
        } else {
            Err(())
        }
    }
    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        if let Some(result) = self
            .in_actions
            .lock()
            .unwrap()
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
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        if let Some(result) = self
            .in_actions
            .lock()
            .unwrap()
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
    fn poll_close(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        if let Some(result) = self
            .in_actions
            .lock()
            .unwrap()
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

pub trait Replicative {
    type Op: Any;

    fn prepare(&mut self, this: Actor, handle: Handle<Self>);
    fn apply(&mut self, op: Self::Op);
}
