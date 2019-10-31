use failure::Fail;
use futures::{
    task::{Context, Poll},
    Stream,
};
use std::{
    collections::BTreeMap,
    fmt::{self, Display, Formatter},
    num::{
        NonZeroI128, NonZeroI16, NonZeroI32, NonZeroI64, NonZeroI8, NonZeroU128, NonZeroU16,
        NonZeroU32, NonZeroU64, NonZeroU8,
    },
    pin::Pin,
};

use crate::{
    cache::{Cache, Sequence},
    clock::Actor,
    Handle, Replicative,
};

#[derive(Fail, Debug)]
pub struct IncrementError;

impl Display for IncrementError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "cannot decrement grow-only counter")
    }
}

pub trait Incrementable: Sized {
    fn increment<I: Into<Self>>(&mut self, by: I) -> Result<(), IncrementError>;
}

macro_rules! impl_primitives {
    ($($ty:ident)+) => {$(
        impl Incrementable for $ty {
            #[allow(unused_comparisons)]
            fn increment<I: Into<Self>>(&mut self, by: I) -> Result<(), IncrementError> {
                let by = by.into();
                if by < 0 {
                    Err(IncrementError)
                } else {
                    *self += by;
                    Ok(())
                }
            }
        }
    )+};
}

impl_primitives!(u8 u16 u32 u64 u128 i8 i16 i32 i64 i128);

macro_rules! impl_nonzero {
    ($($ty:ident)+) => {$(
        impl Incrementable for $ty {
            #[allow(unused_comparisons)]
            fn increment<I: Into<Self>>(&mut self, by: I) ->  Result<(), IncrementError>  {
                let operand = by.into().get();
                if operand < 0 {
                    Err(IncrementError)
                } else {
                    Ok(*self = $ty::new(self.get() + operand).unwrap())
                }
            }
        }
    )+};
}

impl_nonzero!(NonZeroU8 NonZeroU16 NonZeroU32 NonZeroU64 NonZeroU128 NonZeroI8 NonZeroI16 NonZeroI32 NonZeroI64 NonZeroI128);

type Data<T> = BTreeMap<Actor, T>;

pub struct GrowOnly<T: Unpin + Incrementable + Clone> {
    data: Data<T>,
    handle: Sequence<Self>,
    this: Actor,
}

impl<T: Incrementable + Clone + Unpin> GrowOnly<T> {
    pub fn new<I: Into<T>>(item: I) -> Self {
        let handle = Sequence::new();
        let mut data = BTreeMap::new();
        data.insert(Actor::invalid(), item.into());
        GrowOnly {
            handle,
            data,
            this: Actor::invalid(),
        }
    }
    pub fn get(&self) -> T {
        let mut initial = self.data.get(&self.this).unwrap().clone();
        for (actor, counter) in &self.data {
            if actor != &self.this {
                initial.increment(counter.clone()).unwrap();
            }
        }
        initial
    }
    fn increment_origin<I: Into<T>>(&mut self, origin: Actor, by: I) -> Result<(), IncrementError> {
        if let Some(count) = self.data.get_mut(&origin) {
            count.increment(by.into())?;
        } else {
            self.data.insert(origin, by.into());
        }
        Ok(())
    }
    pub fn increment<I: Into<T>>(&mut self, by: I) -> Result<(), IncrementError> {
        if let Some(count) = self.data.get_mut(&self.this) {
            count.increment(by.into())?;
        } else {
            self.data.insert(self.this, by.into());
        }
        Ok(())
    }
}

impl<T: Incrementable + Clone + Unpin> Replicative for GrowOnly<T> {
    type Op = T;
    type MergeError = IncrementError;
    type ApplyError = IncrementError;
    type State = Data<T>;

    fn apply(&mut self, origin: Actor, op: Self::Op) -> Result<(), Self::ApplyError> {
        self.increment_origin(origin, op)
    }
    fn prepare<H: Handle<Self> + 'static>(&mut self, handle: H) {
        if let Some(item) = self.data.remove(&Actor::invalid()) {
            self.data.insert(handle.this().actor(), item);
        }
        self.handle.prepare(handle)
    }
    fn new(state: Self::State) -> Result<Self, Self::MergeError> {
        Ok(GrowOnly {
            this: Actor::invalid(),
            data: state,
            handle: Sequence::new(),
        })
    }
    fn merge(&mut self, state: Self::State) -> Result<(), Self::MergeError> {
        for (actor, item) in state {
            self.increment_origin(actor, item)?;
        }
        Ok(())
    }
    fn fetch(&self) -> Self::State {
        self.data.clone()
    }
}

impl<I: Incrementable + Clone + Unpin> Stream for GrowOnly<I> {
    type Item = <Self as Replicative>::Op;

    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context) -> Poll<Option<Self::Item>> {
        Poll::Ready(self.handle.next_cached())
    }
}
