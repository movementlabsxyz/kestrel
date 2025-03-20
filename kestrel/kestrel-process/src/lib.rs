pub mod fulfill;
pub mod process;
use thiserror::Error;
use tokio::{sync::mpsc::Sender, task::JoinHandle};

#[derive(Debug, Error)]
pub enum ProcessError {
	#[error("failed to run process: {0}")]
	Runtime(#[source] Box<dyn std::error::Error + Send + Sync>),

	#[error("failed to attach pipe: {0}")]
	Pipe(#[source] Box<dyn std::error::Error + Send + Sync>),
}

pub trait ProcessOperations: Sized + Send + Sync + 'static {
	/// Runs the process
	///
	/// It is up to the individual implementation to decide how to the process actually runs.
	fn run(self) -> impl std::future::Future<Output = Result<String, ProcessError>> + Send;

	/// Spawns the process in the background
	///
	/// Kestrel processes should more or less never end and so do not have return values.
	/// For tasks, you can simply write normal Rust.
	fn spawn(self) -> Result<JoinHandle<Result<String, ProcessError>>, ProcessError> {
		let join_handle = tokio::spawn(async move { self.run().await });

		Ok(join_handle)
	}

	/// Attaches a pipe to the process
	///
	/// It is up to the individual implementation to decide how to actually perform the sends within the `run` method.
	fn pipe(&mut self, sender: Sender<String>) -> Result<(), ProcessError>;
}
