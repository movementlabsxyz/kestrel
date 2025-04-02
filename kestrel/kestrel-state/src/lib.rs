use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{Notify, RwLock, RwLockReadGuard, RwLockWriteGuard};
use tokio::time::{sleep, Duration};

/// Main state container holding an optional value.
#[derive(Clone)]
pub struct State<T: Clone + Send + Sync + 'static> {
	inner: Arc<RwLock<Option<T>>>,
	notify: Arc<Notify>,
}

/// Wrapper for writable state
#[derive(Clone)]
pub struct WritableState<T: Clone + Send + Sync + 'static> {
	state: State<T>,
}

/// Wrapper for read-only state
#[derive(Clone)]
pub struct ReadOnlyState<T: Clone + Send + Sync + 'static> {
	state: State<T>,
}

impl<T: Clone + Send + Sync + 'static> State<T> {
	/// Creates a new empty state.
	pub fn new() -> Self {
		Self { inner: Arc::new(RwLock::new(None)), notify: Arc::new(Notify::new()) }
	}

	/// Converts the state into a writable state.
	pub fn write(&self) -> WritableState<T> {
		WritableState { state: self.clone() }
	}

	/// Converts the state into a read-only state.
	pub fn read(&self) -> ReadOnlyState<T> {
		ReadOnlyState { state: self.clone() }
	}
}

impl<T: Clone + Send + Sync + 'static> WritableState<T> {
	/// Returns the write guard for the state.
	pub async fn write(&self) -> RwLockWriteGuard<'_, Option<T>> {
		self.state.inner.write().await
	}

	/// Returns the read guard for the state.
	pub async fn read(&self) -> RwLockReadGuard<'_, Option<T>> {
		self.state.inner.read().await
	}

	/// Writes a value into the state and notifies waiting readers.
	pub async fn set(&self, value: T) {
		let mut lock = self.state.inner.write().await;
		*lock = Some(value);
		self.state.notify.notify_waiters();
	}

	/// Gets a clone of the current value if it's set.
	pub async fn get(&self) -> Option<T> {
		let lock = self.state.inner.read().await;
		lock.clone()
	}
}

/// Error that occurs when waiting for a state to be set.
#[derive(Debug, Error)]
pub enum WaitError {
	#[error("condition not met: {0}")]
	Condition(#[source] Box<dyn std::error::Error + Send + Sync>),
}

pub enum WaitCondition {
	/// Waits up to the given duration
	Duration(Duration),
	/// Waits until the state is set
	Ever,
}

/// Waits until the state is set
pub const EVER: WaitCondition = WaitCondition::Ever;

impl From<Duration> for WaitCondition {
	fn from(duration: Duration) -> Self {
		WaitCondition::Duration(duration)
	}
}

impl<T: Clone + Send + Sync + 'static> ReadOnlyState<T> {
	/// Returns the read guard for the state.
	pub async fn read(&self) -> RwLockReadGuard<'_, Option<T>> {
		self.state.inner.read().await
	}

	/// Waits for the state to be set and returns the value.
	pub async fn wait_forever(&self) -> T {
		loop {
			// First check if the value is already set
			if let Some(value) = self.state.inner.read().await.clone() {
				return value;
			}

			// If not set, prepare to wait
			let notified = self.state.notify.notified();

			// Double-check the value before waiting
			if let Some(value) = self.state.inner.read().await.clone() {
				return value;
			}

			// Now wait for notification
			notified.await;
		}
	}

	/// Waits for the state to be set up to a given duration.
	pub async fn wait_for_duration(&self, duration: Duration) -> Result<T, WaitError> {
		tokio::select! {
			state = self.wait_forever() => {
				Ok(state)
			}
			_ = sleep(duration) => {
				Err(WaitError::Condition("timeout".into()))
			}
		}
	}

	/// Waits for the state to be set up to a given condition.
	pub async fn wait_for(&self, condition: impl Into<WaitCondition>) -> Result<T, WaitError> {
		match condition.into() {
			WaitCondition::Duration(duration) => self.wait_for_duration(duration).await,
			WaitCondition::Ever => Ok(self.wait_forever().await),
		}
	}

	/// Checks if the value is already set.
	pub async fn is_set(&self) -> bool {
		self.state.inner.read().await.is_some()
	}

	/// Gets the current value if it's available.
	pub async fn get(&self) -> Option<T> {
		let lock = self.state.inner.read().await;
		lock.clone()
	}
}

#[cfg(test)]
pub mod test {

	use super::*;

	#[tokio::test]
	async fn test_state_dependency_handling() -> Result<(), anyhow::Error> {
		let a = State::new();
		let b = State::new();

		let writer_a = a.write();
		let writer_b = b.write();
		let reader_a1 = a.read();
		let reader_a2 = a.read();
		let reader_b = b.read();

		let task1: tokio::task::JoinHandle<Result<(String, String), anyhow::Error>> =
			tokio::spawn(async move {
				println!("Task 1 waiting for dependencies...");
				let value_a = reader_a1.wait_forever().await;
				let value_b = reader_b.wait_forever().await;
				println!("Task 1 got: A = {:?}, B = {:?}", value_a, value_b);
				Ok((value_a, value_b)) // Return as Result
			});

		let task2: tokio::task::JoinHandle<Result<String, anyhow::Error>> =
			tokio::spawn(async move {
				println!("Task 2 waiting for A...");
				let value_a = reader_a2.wait_forever().await;
				println!("Task 2 got: A = {:?}", value_a);
				Ok(value_a) // Return as Result
			});

		writer_a.set("Hello".to_string()).await;
		writer_b.set("World".to_string()).await;

		// Handle results properly
		let (value_a_task1, value_b_task1) = task1.await??;
		let value_a_task2 = task2.await??;

		// Assertions
		assert_eq!(value_a_task1, "Hello".to_string());
		assert_eq!(value_b_task1, "World".to_string());
		assert_eq!(value_a_task2, "Hello".to_string());

		Ok(())
	}
}
