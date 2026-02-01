use std::{ops::Deref, sync::OnceLock};

/// A wrapper around OnceLock that panics if accessed before initialization.
/// This is useful for when you know the value will be initialized and want to avoid
/// explicitly calling unwrap() everywhere.
pub(crate) struct UnwrapOnceLock<T>(OnceLock<T>);

impl<T> UnwrapOnceLock<T> {
    pub(crate) const fn new() -> Self {
        UnwrapOnceLock(OnceLock::new())
    }
}

impl<T> UnwrapOnceLock<T> {
    pub(crate) fn init<F>(&self, init: F)
    where
        F: FnOnce() -> T,
    {
        let _ = self.0.get_or_init(init);
    }
}

impl<T> Deref for UnwrapOnceLock<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.get().expect("OnceLock not initialized")
    }
}
