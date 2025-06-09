pub use include_dir::{
	Buildtime as IncludeDirBuildtime, HookError, Noop, PostBuildHook, PreBuildHook,
};
use vendor_util::Vendor;

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
	pub vendor: Vendor,
	/// The include-dir buildtime instance.
	include_dir: IncludeDirBuildtime<Pre, Post>,
}

impl<Pre, Post> Buildtime<Pre, Post>
where
	Pre: PreBuildHook,
	Post: PostBuildHook,
{
	/// Create a new buildtime configuration.
	pub fn try_new(vendor: Vendor) -> Result<Self, BuildtimeError> {
		// Create the include-dir buildtime instance
		let include_dir =
			IncludeDirBuildtime::new(vendor.path.clone(), vendor.plan.vendor_name.clone());

		Ok(Self { vendor, include_dir })
	}

	/// Adds a custom include pattern.
	pub fn include(&mut self, pattern: impl Into<String>) {
		self.include_dir.include(pattern);
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
