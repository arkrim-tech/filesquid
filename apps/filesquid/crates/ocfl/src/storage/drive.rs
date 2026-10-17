//! Run a backend future that never waits, without an executor.
//!
//! [`MemoryBackend`](super::MemoryBackend) and the local filesystem backend do
//! their work on the first poll and never return `Pending`. Synchronous
//! callers (path-based helpers, CLI code, tests) can drive their futures with
//! [`complete_immediately`] instead of pulling in an async runtime. Backends
//! that really wait (S3 over the network) need the host's executor.

use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use super::error::StorageError;

/// Poll `future` once with a no-op waker and return its output.
///
/// # Errors
///
/// [`StorageError::Backend`] when the future is not ready on its first poll;
/// the future is dropped rather than left running unnoticed.
pub fn complete_immediately<F: Future>(future: F) -> Result<F::Output, StorageError> {
    let mut future = pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut cx) {
        Poll::Ready(output) => Ok(output),
        Poll::Pending => Err(StorageError::Backend {
            reason: "the storage future did not complete on its first poll; \
                     drive it with an async executor"
                .to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_futures_return_their_output() {
        assert_eq!(complete_immediately(async { 7 }), Ok(7));
    }

    #[test]
    fn pending_futures_are_reported() {
        let result = complete_immediately(std::future::pending::<()>());
        assert!(matches!(result, Err(StorageError::Backend { .. })));
    }
}
