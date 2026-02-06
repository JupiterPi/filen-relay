use std::{ops::Deref, sync::OnceLock};
#[cfg(feature = "server")]
use tokio::sync::broadcast;

/// A wrapper around OnceLock that panics if accessed before initialization.
/// This is useful for when you know the value will be initialized and want to avoid
/// explicitly calling unwrap() everywhere.
pub struct UnwrapOnceLock<T>(OnceLock<T>);

impl<T> UnwrapOnceLock<T> {
    pub const fn new() -> Self {
        UnwrapOnceLock(OnceLock::new())
    }
}

impl<T> UnwrapOnceLock<T> {
    pub fn init(&self, val: T) {
        let _ = self.0.set(val);
    }
}

impl<T> Deref for UnwrapOnceLock<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.get().expect("OnceLock not initialized")
    }
}

#[cfg(feature = "server")]
pub struct IncrementalVec<T> {
    vec: Vec<T>,
    tx: broadcast::Sender<T>,
}

#[cfg(feature = "server")]
impl<T: Clone> IncrementalVec<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            vec: Vec::with_capacity(capacity),
            tx: broadcast::channel::<T>(capacity).0,
        }
    }

    pub fn push(&mut self, item: T) {
        self.vec.push(item.clone());
        let _ = self.tx.send(item);
    }

    pub fn get(&self) -> (&Vec<T>, broadcast::Receiver<T>) {
        (&self.vec, self.tx.subscribe())
    }
}
