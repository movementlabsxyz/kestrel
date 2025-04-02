pub use kestrel_macro::*;
pub use kestrel_process::*;
pub use kestrel_state::*;
use std::cell::RefCell;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread_local;
use tokio::task::AbortHandle;

/// A unique identifier for tasks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(u64);

/// A task that can be spawned, aborted, and awaited
#[derive(Debug)]
pub struct Task<T> {
	/// Unique identifier for the task
	pub id: TaskId,
	/// Human-readable name for the task
	pub name: String,
	/// The join handle for awaiting the task
	pub handle: JoinHandle<T>,
	/// The abort handle for cancelling the task
	pub abort_handle: AbortHandle,
}

thread_local! {
	static NEXT_TASK_ID: RefCell<AtomicU64> = RefCell::new(AtomicU64::new(0));
}

impl<T> Task<T> {
	/// Creates a new task with the given name
	pub fn new(name: impl Into<String>, handle: JoinHandle<T>, abort_handle: AbortHandle) -> Self {
		let id = NEXT_TASK_ID.with(|next_id| {
			let current = next_id.borrow().load(Ordering::Relaxed);
			next_id.borrow().store(current + 1, Ordering::Relaxed);
			TaskId(current)
		});

		Self { id, name: name.into(), handle, abort_handle }
	}

	/// Aborts the task
	pub fn abort(&self) {
		self.abort_handle.abort();
	}

	/// Awaits the task's completion
	pub async fn await_completion(self) -> Result<T, tokio::task::JoinError> {
		self.handle.await
	}
}

/// Spawns an abortable task and returns a Task struct
pub fn task<F, T>(f: F) -> Task<T>
where
	F: Future<Output = T> + Send + 'static,
	T: Send + 'static,
{
	let handle = tokio::task::spawn(f);
	let abort_handle = handle.abort_handle();

	// Get the caller's location for the task name
	let caller = std::panic::Location::caller();
	let name = format!("task_{}:{}", caller.file(), caller.line());

	Task::new(name, handle, abort_handle)
}

/// Awaits a list of tasks and allows them to be aborted
pub async fn await_allow_abort<T, E>(tasks: Vec<Task<T>>) -> Result<Vec<T>, E>
where
	T: Send + 'static,
	E: From<tokio::task::JoinError>,
{
	let mut results = Vec::with_capacity(tasks.len());
	for task in tasks {
		results.push(task.await_completion().await?);
	}
	Ok(results)
}

/// Aborts a list of tasks
pub fn abort(tasks: &[Task<impl Send + 'static>]) {
	for task in tasks {
		task.abort();
	}
}

/// Aborts all tasks and then awaits their completion
pub async fn end_all<T, E>(tasks: Vec<Task<T>>) -> Result<Vec<T>, E>
where
	T: Send + 'static,
	E: From<tokio::task::JoinError>,
{
	// First abort all tasks
	abort(&tasks);

	// Then await their completion
	await_allow_abort(tasks).await
}

/// Kestrel reuses tokio for basic task management.
pub use tokio::{join, task::JoinHandle, try_join};
