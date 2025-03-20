use jsonlvar::{Jsonl, JsonlError, JsonlParser};
use serde_json::Value;
use std::collections::HashMap;
use thiserror::Error;
use tokio::sync::mpsc::{Receiver, Sender};

#[derive(Debug, Error)]
pub enum JsonlFillerError {
	#[error("Failed to fill variable: {0}")]
	FillError(#[source] Box<dyn std::error::Error + Send + Sync>),
}

pub struct JsonlFiller {
	sender: Sender<String>,
	line_receiver: Receiver<String>,
	line_map: HashMap<String, Value>,
	parser: JsonlParser,
}

impl JsonlFiller {
	pub fn new() -> Self {
		let (sender, line_receiver) = tokio::sync::mpsc::channel(100);
		Self { sender, line_receiver, line_map: HashMap::new(), parser: JsonlParser::new() }
	}

	/// Returns a clone of the sender for sending lines
	pub fn clone_sender(&self) -> Sender<String> {
		self.sender.clone()
	}

	/// Updates the line map by processing received lines
	pub async fn update(&mut self) {
		if let Some(line) = self.line_receiver.recv().await {
			let parsed_vars = self.parser.parse(&line);
			for (key, value) in parsed_vars {
				self.line_map.insert(key, value);
			}
		}
	}

	/// Returns a reference to the line map
	pub fn line_map(&self) -> &HashMap<String, Value> {
		&self.line_map
	}

	/// Tries to fill a variable of type T from the line map
	pub async fn try_fill<T>(
		&mut self,
		var_prefix: Option<&str>,
	) -> Result<Option<T>, JsonlFillerError>
	where
		T: Jsonl,
	{
		self.update().await;
		match T::try_from_jsonl_map(self.line_map(), var_prefix) {
			Ok(value) => Ok(Some(value)),
			Err(JsonlError::MissingField(_)) => Ok(None),
			Err(e) => Err(JsonlFillerError::FillError(Box::new(e))),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde::{Deserialize, Serialize};

	#[derive(Debug, Serialize, Deserialize, PartialEq, Jsonl)]
	struct TestStructInner {
		key: String,
		number: i32,
	}

	#[derive(Debug, Serialize, Deserialize, PartialEq, Jsonl)]
	struct TestStruct {
		key: String,
		number: i32,
		inner: TestStructInner,
	}

	#[tokio::test]
	async fn test_jsonl_filler() -> Result<(), anyhow::Error> {
		let mut filler = JsonlFiller::new();
		let sender = filler.clone_sender();

		let _ = sender.send("JSONL key = value".to_string()).await;
		let result: Option<TestStruct> = filler.try_fill(None).await?;
		assert_eq!(result, None);

		let _ = sender.send("JSONL number = 42".to_string()).await;
		let result: Option<TestStruct> = filler.try_fill(None).await?;
		assert_eq!(result, None);

		let _ = sender
			.send("JSONL inner = {\"key\": \"value\", \"number\": 42}".to_string())
			.await;
		let result: Option<TestStruct> = filler.try_fill(None).await?;

		assert_eq!(
			result,
			Some(TestStruct {
				key: "value".to_string(),
				number: 42,
				inner: TestStructInner { key: "value".to_string(), number: 42 },
			})
		);
		Ok(())
	}
}
