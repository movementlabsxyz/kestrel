use crate::fulfill::{Fulfill, FulfillError};
use jsonlvar::Jsonl as JsonlOperations;
use jsonlvar_tokio::JsonlFiller;
use kestrel_state::WritableState;
use std::future::Future;
use tokio::sync::mpsc::Sender;

/// A fulfiller that fulfills requests using JSONL.
pub struct Jsonl<T>
where
	T: JsonlOperations + Clone + Send + Sync + 'static,
{
	filler: JsonlFiller,
	dependency: WritableState<T>,
	var_prefix: Option<String>,
}

impl<T> Jsonl<T>
where
	T: JsonlOperations + Clone + Send + Sync + 'static,
{
	/// Creates a new Jsonl fulfiller.
	pub fn new(dependency: WritableState<T>, var_prefix: Option<String>) -> Self {
		Self { filler: JsonlFiller::new(), dependency, var_prefix }
	}
}

impl<T> Fulfill<T> for Jsonl<T>
where
	T: JsonlOperations + Clone + Send + Sync + 'static,
{
	fn sender(&self) -> Result<Sender<String>, FulfillError> {
		Ok(self.filler.clone_sender())
	}

	fn dependency(&self) -> Result<WritableState<T>, FulfillError> {
		Ok(self.dependency.clone())
	}

	fn try_get(&mut self) -> impl Future<Output = Result<Option<T>, FulfillError>> + Send {
		async move {
			self.filler
				.try_fill(self.var_prefix.as_deref())
				.await
				.map_err(|e| FulfillError::Fulfill(Box::new(e)))
		}
	}
}
