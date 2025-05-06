use ignore::WalkBuilder;
use std::collections::HashSet;
use std::env;
use std::fmt::Debug;
use std::fs::File;
use std::io::BufWriter;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use zip::{write::SimpleFileOptions, ZipWriter};

#[derive(Debug, thiserror::Error)]
pub enum HookError {
	#[error("internal error: {0}")]
	Internal(#[source] Box<dyn std::error::Error + Send + Sync>),
}

pub trait PreBuildHook: Debug + Clone {
	fn before(&self) -> Result<(), HookError>;
}

pub trait PostBuildHook: Debug + Clone {
	fn after(&self) -> Result<(), HookError>;
}

#[derive(Debug, Clone)]
pub struct Noop;

impl PreBuildHook for Noop {
	fn before(&self) -> Result<(), HookError> {
		Ok(())
	}
}

impl PostBuildHook for Noop {
	fn after(&self) -> Result<(), HookError> {
		Ok(())
	}
}

#[derive(Debug, thiserror::Error)]
pub enum BuildtimeError {
	#[error("internal error: {0}")]
	Internal(#[source] Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug, Clone)]
pub struct Buildtime<Pre = Noop, Post = Noop>
where
	Pre: PreBuildHook,
	Post: PostBuildHook,
{
	directory_path: PathBuf,
	name: String,
	include_patterns: HashSet<String>,
	pre_build_hooks: Vec<Pre>,
	post_build_hooks: Vec<Post>,
}

impl<Pre, Post> Buildtime<Pre, Post>
where
	Pre: PreBuildHook,
	Post: PostBuildHook,
{
	pub fn new(directory_path: PathBuf, name: String) -> Self {
		Self {
			directory_path,
			name,
			include_patterns: HashSet::new(),
			pre_build_hooks: Vec::new(),
			post_build_hooks: Vec::new(),
		}
	}

	/// Adds a custom include pattern.
	pub fn include(&mut self, pattern: impl Into<String>) {
		self.include_patterns.insert(pattern.into());
	}

	/// Adds a pre-build hook.
	pub fn before(&mut self, hook: Pre) {
		self.pre_build_hooks.push(hook);
	}

	/// Adds a post-build hook.
	pub fn after(&mut self, hook: Post) {
		self.post_build_hooks.push(hook);
	}
	/// Builds the directory into a zip file.
	pub fn build(&self) -> Result<(), BuildtimeError> {
		// Run the pre-build hooks
		for hook in &self.pre_build_hooks {
			hook.before().map_err(|e| BuildtimeError::Internal(e.into()))?;
		}

		// Define the source directory (relative to the crate)
		if !self.directory_path.exists() {
			return Err(BuildtimeError::Internal(Box::new(std::io::Error::new(
				std::io::ErrorKind::NotFound,
				format!("source directory {:?} does not exist!", self.directory_path),
			))));
		}

		// Get the output directory where build artifacts are stored
		let out_dir = env::var("OUT_DIR").unwrap();
		let zip_path = Path::new(&out_dir).join(format!("{}.zip", self.name));

		// Create the zip file
		let zip_file = File::create(&zip_path).map_err(|e| BuildtimeError::Internal(e.into()))?;
		let mut zip = ZipWriter::new(BufWriter::new(zip_file));

		// Create an ignore walker with overrides
		let mut builder = WalkBuilder::new(self.directory_path.clone());
		builder.git_ignore(true).git_exclude(true).hidden(false);

		let walker = builder.build();

		// Create a separate walker for explicitly included files
		let mut explicit_builder = WalkBuilder::new(self.directory_path.clone());
		explicit_builder.git_ignore(false).git_exclude(false).hidden(true);

		// Add custom include patterns
		if !self.include_patterns.is_empty() {
			for pattern in &self.include_patterns {
				explicit_builder.add(pattern);
			}
		}

		let explicit_walker = explicit_builder.build();

		// Create a HashSet to track processed paths
		let mut processed_paths = HashSet::new();

		// First process git-tracked files
		for entry in walker.filter_map(Result::ok) {
			let path = entry.path();
			let name = path.strip_prefix(&self.directory_path).unwrap().to_str().unwrap();
			processed_paths.insert(name.to_string());

			if path.is_file() {
				// Get the file's Unix permissions
				let metadata = path.metadata().map_err(|e| BuildtimeError::Internal(e.into()))?;
				let mode = metadata.permissions().mode();

				// Create options with Unix permissions
				let options = SimpleFileOptions::default()
					.compression_method(zip::CompressionMethod::Stored)
					.unix_permissions(mode);

				let mut file = File::open(path).map_err(|e| BuildtimeError::Internal(e.into()))?;
				zip.start_file(name, options).map_err(|e| BuildtimeError::Internal(e.into()))?;
				std::io::copy(&mut file, &mut zip)
					.map_err(|e| BuildtimeError::Internal(e.into()))?;
			} else if path.is_dir() {
				// Get the directory's Unix permissions
				let metadata = path.metadata().map_err(|e| BuildtimeError::Internal(e.into()))?;
				let mode = metadata.permissions().mode();

				// Create options with Unix permissions
				let options = SimpleFileOptions::default()
					.compression_method(zip::CompressionMethod::Stored)
					.unix_permissions(mode);

				zip.add_directory(name, options)
					.map_err(|e| BuildtimeError::Internal(e.into()))?;
			}
		}

		// Then process explicitly included files that weren't already processed
		for entry in explicit_walker.filter_map(Result::ok) {
			let path = entry.path();
			let name = path.strip_prefix(&self.directory_path).unwrap().to_str().unwrap();

			// Skip if we already processed this path
			if processed_paths.contains(name) {
				continue;
			}

			if path.is_file() {
				// Get the file's Unix permissions
				let metadata = path.metadata().map_err(|e| BuildtimeError::Internal(e.into()))?;
				let mode = metadata.permissions().mode();

				// Create options with Unix permissions
				let options = SimpleFileOptions::default()
					.compression_method(zip::CompressionMethod::Stored)
					.unix_permissions(mode);

				let mut file = File::open(path).map_err(|e| BuildtimeError::Internal(e.into()))?;
				zip.start_file(name, options).map_err(|e| BuildtimeError::Internal(e.into()))?;
				std::io::copy(&mut file, &mut zip)
					.map_err(|e| BuildtimeError::Internal(e.into()))?;
			} else if path.is_dir() {
				// Get the directory's Unix permissions
				let metadata = path.metadata().map_err(|e| BuildtimeError::Internal(e.into()))?;
				let mode = metadata.permissions().mode();

				// Create options with Unix permissions
				let options = SimpleFileOptions::default()
					.compression_method(zip::CompressionMethod::Stored)
					.unix_permissions(mode);

				zip.add_directory(name, options)
					.map_err(|e| BuildtimeError::Internal(e.into()))?;
			}
		}

		zip.finish().map_err(|e| BuildtimeError::Internal(e.into()))?;

		// Run the post-build hooks
		for hook in &self.post_build_hooks {
			hook.after().map_err(|e| BuildtimeError::Internal(e.into()))?;
		}

		println!("cargo:rerun-if-changed={}", self.directory_path.display());

		Ok(())
	}
}
