use cargo_metadata::MetadataCommand;
pub use include_dir::{Buildtime as IncludeDirBuildtime, Noop, PostBuildHook, PreBuildHook};
use std::collections::HashSet;
use std::path::PathBuf;

/// Error type for buildtime operations.
#[derive(Debug, thiserror::Error)]
pub enum BuildtimeError {
	#[error("Internal error: {0}")]
	Internal(#[from] anyhow::Error),
}

/// Buildtime configuration for vendor paths.
pub struct Buildtime<Pre = Noop, Post = Noop>
where
	Pre: PreBuildHook,
	Post: PostBuildHook,
{
	/// The name of the vendor.
	pub vendor_name: String,
	/// The include-dir buildtime instance.
	include_dir: IncludeDirBuildtime<Pre, Post>,
}

impl<Pre, Post> Buildtime<Pre, Post>
where
	Pre: PreBuildHook,
	Post: PostBuildHook,
{
	/// Create a new buildtime configuration.
	pub fn try_new(
		vendor_name: impl Into<String>,
		include_patterns: HashSet<String>,
		pre_build_hooks: Vec<Pre>,
		post_build_hooks: Vec<Post>,
	) -> Result<Self, BuildtimeError> {
		let vendor_name = vendor_name.into();

		// Get the workspace root using cargo_metadata
		let metadata =
			MetadataCommand::new().exec().map_err(|e| BuildtimeError::Internal(e.into()))?;
		let workspace_root = metadata.workspace_root;

		// Construct the path to the vendor directory from workspace root
		let vendor_path = PathBuf::from(workspace_root).join(".vendors").join(&vendor_name);

		// Create the include-dir buildtime instance
		let include_dir = IncludeDirBuildtime::new(
			vendor_path,
			vendor_name.clone(),
			include_patterns,
			pre_build_hooks,
			post_build_hooks,
		);

		Ok(Self { vendor_name, include_dir })
	}

	/// Adds a pre-build hook.
	pub fn before(&mut self, hook: Pre) {
		self.include_dir.before(hook);
	}

	/// Adds a post-build hook.
	pub fn after(&mut self, hook: Post) {
		self.include_dir.after(hook);
	}

	/// Build the vendor directory.
	pub fn build(&self) -> Result<(), BuildtimeError> {
		self.include_dir.build().map_err(|e| BuildtimeError::Internal(e.into()))
	}
}
