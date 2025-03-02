use std::ffi::{OsStr, OsString};

use super::ServerRef;

/// Reference to a unix socket server instance
pub struct WindowsPipeServerRef {
    pub(crate) addr: OsString,
    pub(crate) inner: Box<dyn ServerRef>,
}

impl WindowsPipeServerRef {
    pub fn new(addr: OsString, inner: Box<dyn ServerRef>) -> Self {
        Self { addr, inner }
    }

    /// Returns the addr that the listener is bound to
    pub fn addr(&self) -> &OsStr {
        &self.addr
    }

    /// Consumes ref, returning inner ref
    pub fn into_inner(self) -> Box<dyn ServerRef> {
        self.inner
    }
}

impl ServerRef for WindowsPipeServerRef {
    fn is_finished(&self) -> bool {
        self.inner.is_finished()
    }

    fn shutdown(&self) {
        self.inner.shutdown();
    }
}
