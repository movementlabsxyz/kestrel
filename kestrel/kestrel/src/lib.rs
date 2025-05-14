use futures::future::{AbortHandle, Abortable, Aborted};
pub use kestrel_macro::*;
pub use kestrel_process::*;
pub use kestrel_state::*;
use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::AtomicU64;
use std::task::{Context, Poll};
use std::thread_local;
use tokio::task::JoinHandle;

/// Errors thrown by the Task struct.
#[derive(Debug, thiserror::Error)]
pub enum TaskError {
	#[error("task aborted: {0}")]
	Aborted(#[source] Aborted),
	#[error("join error: {0}")]
	Join(#[source] tokio::task::JoinError),
	#[error("multiple errors encountered across tasks: {0:?}")]
	MultipleErrors(Vec<TaskError>),
}

/// A value that may be aborted
#[derive(Debug)]
pub enum Maybe<T> {
	Value(T),
	Aborted(Aborted),
}

/// A unique identifier for tasks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(u64);

/// A task that can be spawned, aborted, and awaited
#[derive(Debug)]
pub struct Task<T> {
	/// The join handle for awaiting the task
	pub handle: JoinHandle<Result<T, Aborted>>,
	/// The abort handle for cancelling the task
	pub abort_handle: AbortHandle,
}

impl<T> Task<T> {
	/// Aborts the task
	pub fn abort(&self) {
		self.abort_handle.abort();
		self.handle.abort();
	}

	/// Returns whether the task has been aborted
	pub fn is_aborted(&self) -> bool {
		self.abort_handle.is_aborted()
	}

	/// Awaits a task, but allows an abort by wrapping as a [Maybe]
	pub async fn maybe(self) -> Result<Maybe<T>, TaskError> {
		match self.await {
			Ok(result) => Ok(Maybe::Value(result)),
			Err(e) => match e {
				TaskError::Aborted(e) => Ok(Maybe::Aborted(e)),
				TaskError::Join(e) => Err(TaskError::Join(e)),
				TaskError::MultipleErrors(e) => Err(TaskError::MultipleErrors(e)),
			},
		}
	}

	/// Awaits a task, but allows an abort
	pub async fn await_allow_abort(self) -> Result<(), TaskError> {
		match self.maybe().await {
			Ok(_) => Ok(()),
			Err(TaskError::Join(join_error)) if join_error.is_cancelled() => {
				// If the task was cancelled via its JoinHandle (which our Task::abort now does),
				// consider it a successful "end" for the purposes of this function.
				Ok(())
			}
			Err(e) => Err(e), // Other errors (like panics or non-cancellation JoinErrors) are still errors.
		}
	}
}

/// In contrast to tokio's task, this task will abort when dropped
///
/// This means you have to hold the task handle to ensure the task is not aborted
/// when the task handle is dropped.
impl<T> Drop for Task<T> {
	fn drop(&mut self) {
		self.abort();
	}
}

impl<T> Future for Task<T> {
	type Output = Result<T, TaskError>;

	fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		match Pin::new(&mut self.handle).poll(cx) {
			Poll::Pending => Poll::Pending,
			Poll::Ready(Ok(result)) => match result {
				Ok(result) => Poll::Ready(Ok(result)),
				Err(e) => Poll::Ready(Err(TaskError::Aborted(e))),
			},
			Poll::Ready(Err(e)) => Poll::Ready(Err(TaskError::Join(e))),
		}
	}
}

/// Spawns an abortable task and returns a Task struct
pub fn task<F, T>(f: F) -> Task<T>
where
	F: Future<Output = T> + Send + 'static,
	T: Send + 'static,
{
	let (abort_handle, abort_reg) = AbortHandle::new_pair();
	let handle = tokio::task::spawn(Abortable::new(f, abort_reg));

	Task { handle, abort_handle }
}

/// Awaits multiple tasks but allows them to abort
#[macro_export]
macro_rules! await_allow_abort {
    ($($task:expr),* $(,)?) => {{
        let mut result = Ok(());
        $(
            if result.is_ok() {
                result = $task.await_allow_abort().await;
            }
        )*
        result
    }};
}

/// Aborts multiple tasks
#[macro_export]
macro_rules! abort {
    ($($task:expr),* $(,)?) => {
        {
            $(
                $task.abort();
            )*
        }
    };
}

#[macro_export]
macro_rules! end {
    ($($task:expr),* $(,)?) => {{
        let mut result = Ok(());
        $(
            $task.abort();
        )*
        $(
            if result.is_ok() {
                result = $task.await_allow_abort().await;
            }
        )*
        result
    }};
}
