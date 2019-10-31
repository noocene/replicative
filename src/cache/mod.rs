use crate::{Handle, Replicative};

pub mod sequence;
pub mod single;
pub use sequence::Sequence;
pub use single::Single;

pub trait Cache<T: Replicative> {
    fn prepare<H: Handle<T> + 'static>(&mut self, handle: H);
    fn dispatch(&mut self, op: T::Op);
    fn next_cached(&mut self) -> Option<T::Op>;
}
