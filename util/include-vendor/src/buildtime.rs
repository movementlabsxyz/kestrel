use cargo_metadata::MetadataCommand;
use include_dir::Buildtime as IncludeDirBuildtime;
use std::path::PathBuf;

/// Error type for buildtime operations.
#[derive(Debug, thiserror::Error)]
pub enum BuildtimeError {
	#[error("Internal error: {0}")]
	Internal(#[from] anyhow::Error),
}

/// Buildtime configuration for vendor paths.
pub struct Buildtime {
	/// The name of the vendor.
	pub vendor_name: String,
	/// The include-dir buildtime instance.
	include_dir: IncludeDirBuildtime,
}

impl Buildtime {
	/// Create a new buildtime configuration.
	pub fn try_new(vendor_name: impl Into<String>) -> Result<Self, BuildtimeError> {
		let vendor_name = vendor_name.into();

		// Get the workspace root using cargo_metadata
		let metadata =
			MetadataCommand::new().exec().map_err(|e| BuildtimeError::Internal(e.into()))?;
		let workspace_root = metadata.workspace_root;

		// Construct the path to the vendor directory from workspace root
		let vendor_path = PathBuf::from(workspace_root).join(".vendors").join(&vendor_name);

		// Create the include-dir buildtime instance
		let include_dir = IncludeDirBuildtime::new(vendor_path, vendor_name.clone());

		Ok(Self { vendor_name, include_dir })
	}

	/// Build the vendor directory.
	pub fn build(&self) -> Result<(), BuildtimeError> {
		self.include_dir.build().map_err(|e| BuildtimeError::Internal(e.into()))
	}
}
