pub mod custom;
pub mod jsonl;

use kestrel_state::WritableState;
use std::future::Future;
use thiserror::Error;
use tokio::sync::mpsc::Sender;

#[derive(Debug, Error)]
pub enum FulfillError {
	#[error("failed to fulfill request: {0}")]
	Fulfill(#[source] Box<dyn std::error::Error + Send + Sync>),

	#[error("failed to get sender: {0}")]
	Sender(#[source] Box<dyn std::error::Error + Send + Sync>),

	#[error("internal fulfillment error: {0}")]
	Internal(#[source] Box<dyn std::error::Error + Send + Sync>),
}

pub trait Fulfill<T>: Sized + Send + Sync + 'static
where
	T: Clone + Send + Sync + 'static,
{
	/// Gets the sender that will be used to fulfill the request via the pipe.
	fn sender(&self) -> Result<Sender<String>, FulfillError>;

	/// Gets the writable state value which is supposed to be fulfilled.
	fn dependency(&self) -> Result<WritableState<T>, FulfillError>;

	/// Attempts to get the value to fulfill the request.
	fn try_get(&mut self) -> impl Future<Output = Result<Option<T>, FulfillError>> + Send;

	/// Attempts to update the writable state value with the fulfilled value.
	fn try_fulfill(&mut self) -> impl Future<Output = Result<T, FulfillError>> + Send {
		async {
			match self.try_get().await? {
				Some(value) => {
					self.dependency()?.set(value.clone()).await;
					Ok(value)
				}
				None => Err(FulfillError::Fulfill("unable to fulfill request".into())),
			}
		}
	}

	/// Runs the fulfillment task
	fn run(mut self) -> impl Future<Output = Result<T, FulfillError>> + Send {
		async move {
			loop {
				match self.try_fulfill().await {
					Ok(value) => return Ok(value),
					Err(FulfillError::Fulfill(_)) => {
						// continue waiting for fulfillment
						continue;
					}
					Err(e) => return Err(e),
				}
			}
		}
	}

	/// Spawns the fulfillment task in the background
	fn spawn(self) -> Result<tokio::task::JoinHandle<Result<T, FulfillError>>, FulfillError> {
		let join_handle = tokio::spawn(async move { self.run().await });

		Ok(join_handle)
	}
}
