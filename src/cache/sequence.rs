use crate::{Handle, Replicative};

use super::Cache;

pub enum Sequence<T: Replicative> {
    Cache(Vec<T::Op>),
    Handle(Box<dyn Handle<T>>),
}

impl<T: Replicative> Cache<T> for Sequence<T>
where
    T::Op: Clone,
{
    fn prepare<H: Handle<T> + 'static>(&mut self, mut handle: H) {
        use Sequence::{Cache, Handle};
        if let Cache(items) = self {
            for item in items {
                handle.dispatch(item.clone());
            }
        }
        *self = Handle(Box::new(handle))
    }
    fn dispatch(&mut self, op: T::Op) {
        use Sequence::{Cache, Handle};
        match self {
            Cache(items) => items.push(op),
            Handle(handle) => handle.dispatch(op),
        }
    }
    fn next_cached(&mut self) -> Option<T::Op> {
        use Sequence::Cache;
        if let Cache(items) = self {
            return items.pop();
        }
        None
    }
}

impl<T: Replicative> Sequence<T>
where
    T::Op: Clone,
{
    pub fn new() -> Self {
        use Sequence::Cache;
        Cache(vec![])
    }
}
