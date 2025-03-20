use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use thiserror::Error;

pub struct JsonlParser {
	// Placeholder for future configurable options
}

impl JsonlParser {
	pub fn new() -> Self {
		JsonlParser {}
	}

	pub fn parse(&self, input: &str) -> HashMap<String, Value> {
		let mut map = HashMap::new();
		let re = Regex::new(r"JSONL\s+(\w+)\s*=\s*(.+)$").unwrap();

		for line in input.lines() {
			if let Some(caps) = re.captures(line) {
				let var_name = caps.get(1).unwrap().as_str().to_string();
				let value_str = caps.get(2).unwrap().as_str().trim();

				// Try parsing as JSON first
				let parsed_value = match serde_json::from_str::<Value>(value_str) {
					Ok(json_value) => json_value,
					Err(_) => {
						// If JSON parsing fails, assume it's a raw string or number
						if let Ok(number) = value_str.parse::<f64>() {
							Value::from(number) // Store numbers as JSON numbers
						} else {
							Value::from(value_str.to_string()) // Store strings as JSON strings
						}
					}
				};

				map.insert(var_name, parsed_value);
			}
		}

		map
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_jsonl_parser() {
		let input = r#"
        Random log entry
        JSONL foo = {"key": "value"}
        JSONL bar = {"number": 42, "list": [1, 2, 3]}
        JSONL raw_string = HelloWorld
        JSONL raw_number = 12345
        JSONL invalid = {invalid json gets parsed as string}
        "#;

		let parser = JsonlParser::new();
		let result = parser.parse(input);

		assert_eq!(result.len(), 5);
		assert_eq!(result.get("foo").unwrap(), &serde_json::json!({"key": "value"}));
		assert_eq!(
			result.get("bar").unwrap(),
			&serde_json::json!({"number": 42, "list": [1, 2, 3]})
		);
		assert_eq!(result.get("raw_string").unwrap(), &serde_json::json!("HelloWorld"));
		assert_eq!(result.get("raw_number").unwrap(), &serde_json::json!(12345));
		assert_eq!(
			result.get("invalid").unwrap(),
			&serde_json::json!("{invalid json gets parsed as string}")
		);
	}
}

#[derive(Debug, Error)]
pub enum JsonlError {
	#[error("JSON parsing error: {0}")]
	Json(#[from] serde_json::Error),

	#[error("Missing or invalid field: {0}")]
	MissingField(String),
}

pub trait Jsonl: Sized + Serialize {
	/// Converts a parsed JSONL map into the struct
	fn try_from_jsonl_map(
		parsed_data: &HashMap<String, Value>,
		var_prefix: Option<&str>,
	) -> Result<Self, JsonlError>;

	/// Parses a JSONL string into a struct
	fn try_from_jsonl(jsonl: &str, var_prefix: Option<&str>) -> Result<Self, JsonlError> {
		let parser = JsonlParser::new();
		let parsed_data = parser.parse(jsonl);
		Self::try_from_jsonl_map(&parsed_data, var_prefix)
	}

	/// Converts the struct into a JSONL-formatted string with a variable name
	fn try_to_jsonl(&self, var_name: &str) -> Result<String, JsonlError> {
		let serialized = serde_json::to_string(self)?;
		Ok(format!("JSONL {} = {}", var_name, serialized))
	}

	/// Converts each field of the struct into a list of individual JSONL entries
	fn try_to_jsonl_flat_vec(&self, var_prefix: Option<String>) -> Result<Vec<String>, JsonlError>;

	/// Converts each field of the struct into a single JSONL-formatted string (newline-separated)
	fn try_to_jsonl_flat(&self, var_prefix: Option<String>) -> Result<String, JsonlError> {
		let entries = self.try_to_jsonl_flat_vec(var_prefix)?;
		Ok(entries.join("\n"))
	}
}
