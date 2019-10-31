pub mod grow_only;
pub use grow_only::GrowOnly;

use crate::{clock::Actor, Handle, Replicative};

use failure::Fail;
use futures::{
    task::{Context, Poll},
    Stream,
};
use std::{
    fmt::{self, Display, Formatter},
    num::{
        NonZeroI128, NonZeroI16, NonZeroI32, NonZeroI64, NonZeroI8, NonZeroU128, NonZeroU16,
        NonZeroU32, NonZeroU64, NonZeroU8,
    },
    ops::{Neg, Sub},
    pin::Pin,
};
use void::Void;

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

pub struct Counter<T: Unpin + Incrementable + Clone> {
    p: Pin<Box<GrowOnly<T>>>,
    n: Pin<Box<GrowOnly<T>>>,
}

pub trait Zero {
    fn zero() -> Self;
}

macro_rules! impl_primitives {
    ($($ty:ident)+) => {$(
        impl Zero for $ty {
            fn zero() -> Self {
                0
            }
        }
    )+};
}

impl_primitives!(u8 u16 u32 u64 u128 i8 i16 i32 i64 i128);

impl<
        T: Incrementable + Clone + Unpin + Zero + Sub<T, Output = T> + Neg<Output = T> + PartialOrd,
    > Counter<T>
{
    pub fn new(item: T) -> Self {
        Counter {
            p: Box::pin(GrowOnly::new(item)),
            n: Box::pin(GrowOnly::new(T::zero())),
        }
    }
    pub fn get(&self) -> T {
        self.p.get() - self.n.get()
    }
    fn increment_origin<I: Into<T>>(&mut self, origin: Actor, by: I) -> Result<(), Void> {
        let item = by.into();
        if item > T::zero() {
            self.p.increment_origin(origin, item).unwrap()
        } else {
            self.n.increment_origin(origin, -item).unwrap()
        }
        Ok(())
    }
    pub fn add<I: Into<T>>(&mut self, by: I) -> Result<(), IncrementError> {
        let item = by.into();
        if item > T::zero() {
            self.p.increment(item).unwrap()
        } else {
            self.n.increment(-item).unwrap()
        }
        Ok(())
    }
    pub fn sub<I: Into<T>>(&mut self, by: I) -> Result<(), IncrementError> {
        self.add(-by.into())
    }
}

impl<
        T: Incrementable + Clone + Zero + Sub<T, Output = T> + Neg<Output = T> + PartialOrd + Unpin,
    > Replicative for Counter<T>
{
    type Op = T;
    type MergeError = IncrementError;
    type ApplyError = IncrementError;
    type Target = GrowOnly<T>;
    type State = (
        <GrowOnly<T> as Replicative>::State,
        <GrowOnly<T> as Replicative>::State,
    );
    fn apply(&mut self, origin: Actor, op: Self::Op) -> Result<(), Self::ApplyError> {
        Ok(self.increment_origin(origin, op).unwrap())
    }
    fn prepare<H: Handle<Self::Target> + 'static>(&mut self, handle: H) {
        self.n.prepare(handle);
    }
    fn new(state: Self::State) -> Result<Self, Self::MergeError> {
        let mut counter = Counter {
            p: Box::pin(GrowOnly::new(T::zero())),
            n: Box::pin(GrowOnly::new(T::zero())),
        };
        counter.p.merge(state.0)?;
        counter.n.merge(state.1)?;
        Ok(counter)
    }
    fn merge(&mut self, state: Self::State) -> Result<(), Self::MergeError> {
        self.p.merge(state.0)?;
        self.n.merge(state.1)?;
        Ok(())
    }
    fn fetch(&self) -> Self::State {
        (self.p.fetch(), self.n.fetch())
    }
}

impl<
        I: Incrementable + Clone + Unpin + Zero + Sub<I, Output = I> + Neg<Output = I> + PartialOrd,
    > Stream for Counter<I>
{
    type Item = <Self as Replicative>::Op;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        if let Poll::Ready(Some(item)) = self.p.as_mut().poll_next(cx) {
            Poll::Ready(Some(item))
        } else if let Poll::Ready(Some(item)) = self.n.as_mut().poll_next(cx) {
            Poll::Ready(Some(-item))
        } else {
            Poll::Ready(None)
        }
    }
}
