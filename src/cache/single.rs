use crate::{Handle, Replicative};

use super::Cache;

pub enum Single<T: Replicative> {
    Cache(Option<T::Op>),
    Handle(Box<dyn Handle<T>>),
}

impl<T: Replicative> Cache<T> for Single<T>
where
    T::Op: Clone,
{
    fn prepare<H: Handle<T> + 'static>(&mut self, mut handle: H) {
        use Single::{Cache, Handle};
        if let Cache(items) = self {
            for item in items {
                handle.dispatch(item.clone());
            }
        }
        *self = Handle(Box::new(handle))
    }
    fn dispatch(&mut self, op: T::Op) {
        use Single::{Cache, Handle};
        match self {
            Cache(item) => {
                *item = Some(op);
            }
            Handle(handle) => handle.dispatch(op),
        }
    }
    fn next_cached(&mut self) -> Option<T::Op> {
        use Single::Cache;
        if let Cache(item) = self {
            return item.take();
        }
        None
    }
}

impl<T: Replicative> Single<T>
where
    T::Op: Clone,
{
    pub fn new() -> Self {
        use Single::Cache;
        Cache(None)
    }
}
