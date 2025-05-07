use bollard::image::{CreateImageOptions, ListImagesOptions};
use bollard::Docker;
use futures::StreamExt;
use std::collections::{HashMap, HashSet};

#[derive(Debug, thiserror::Error)]
pub enum HookError {
	#[error("internal error: {0}")]
	Internal(#[source] Box<dyn std::error::Error + Send + Sync>),
}

pub trait PreBuildHook: std::fmt::Debug + Clone {
	fn before(&self) -> Result<(), HookError>;
}

pub trait PostBuildHook: std::fmt::Debug + Clone {
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
	images: HashSet<String>,
	pre_build_hooks: Vec<Pre>,
	post_build_hooks: Vec<Post>,
}

impl<Pre, Post> Buildtime<Pre, Post>
where
	Pre: PreBuildHook,
	Post: PostBuildHook,
{
	pub fn new() -> Self {
		Self { images: HashSet::new(), pre_build_hooks: Vec::new(), post_build_hooks: Vec::new() }
	}

	/// Add an image to be pulled
	pub fn add_image(&mut self, image: impl Into<String>) -> &mut Self {
		self.images.insert(image.into());
		self
	}

	/// Add a pre-build hook
	pub fn before(&mut self, hook: Pre) {
		self.pre_build_hooks.push(hook);
	}

	/// Add a post-build hook
	pub fn after(&mut self, hook: Post) {
		self.post_build_hooks.push(hook);
	}

	/// Build the Docker images, ensuring they are available
	pub async fn build(&self) -> Result<(), BuildtimeError> {
		// Run pre-build hooks
		for hook in &self.pre_build_hooks {
			hook.before().map_err(|e| BuildtimeError::Internal(e.into()))?;
		}

		let docker = Docker::connect_with_local_defaults()
			.map_err(|e| BuildtimeError::Internal(e.into()))?;

		for image in &self.images {
			// Check if image already exists
			let mut filters = HashMap::new();
			filters.insert("reference".to_string(), vec![image.to_string()]);
			let options = ListImagesOptions { filters, ..Default::default() };

			let images = docker
				.list_images(Some(options))
				.await
				.map_err(|e| BuildtimeError::Internal(e.into()))?;

			if images.is_empty() {
				let options =
					CreateImageOptions { from_image: image.to_string(), ..Default::default() };

				let mut stream = docker.create_image(Some(options), None, None);
				while let Some(msg) = stream.next().await {
					match msg {
						Ok(msg) => {
							if let Some(status) = msg.status {
								println!("cargo:warning=Docker: {}", status);
							}
						}
						Err(e) => {
							return Err(BuildtimeError::Internal(e.into()));
						}
					}
				}
			}
		}

		// Run post-build hooks
		for hook in &self.post_build_hooks {
			hook.after().map_err(|e| BuildtimeError::Internal(e.into()))?;
		}

		Ok(())
	}
}
