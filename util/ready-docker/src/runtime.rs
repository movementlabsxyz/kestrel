use bollard::image::{CreateImageOptions, ListImagesOptions};
use bollard::Docker;
use futures::StreamExt;
use std::collections::HashMap;
use std::ffi::OsStr;

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
	#[error("internal error: {0}")]
	Internal(#[source] Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug)]
pub struct Runtime {
	docker: Docker,
}

impl Runtime {
	/// Create a new Runtime instance
	pub async fn new() -> Result<Self, RuntimeError> {
		let docker =
			Docker::connect_with_local_defaults().map_err(|e| RuntimeError::Internal(e.into()))?;
		Ok(Self { docker })
	}

	/// Check if an image exists locally
	pub async fn image_exists(&self, image: &str) -> Result<bool, RuntimeError> {
		let mut filters = HashMap::new();
		filters.insert("reference", vec![image]);
		let options = ListImagesOptions { filters, ..Default::default() };

		let images = self
			.docker
			.list_images(Some(options))
			.await
			.map_err(|e| RuntimeError::Internal(e.into()))?;
		Ok(!images.is_empty())
	}

	/// Pull an image if it doesn't exist
	pub async fn ensure_image(&self, image: &str) -> Result<(), RuntimeError> {
		if !self.image_exists(image).await? {
			let options = CreateImageOptions { from_image: image, ..Default::default() };

			let mut stream = self.docker.create_image(Some(options), None, None);
			while let Some(msg) = stream.next().await {
				match msg {
					Ok(msg) => {
						if let Some(status) = msg.status {
							println!("Docker: {}", status);
						}
					}
					Err(e) => {
						return Err(RuntimeError::Internal(e.into()));
					}
				}
			}
		}
		Ok(())
	}

	/// Constructs a command to run in the Docker environment
	pub fn command<C, I, S>(&self, command: C, args: I) -> commander::Command
	where
		C: AsRef<OsStr>,
		I: IntoIterator<Item = S>,
		S: AsRef<OsStr>,
	{
		let mut cmd = commander::Command::new(command, true, vec![], vec![]);
		cmd.args(args);
		cmd
	}

	/// Runs a command in the Docker environment
	pub async fn run_command<C, I, S>(&self, command: C, args: I) -> Result<String, RuntimeError>
	where
		C: AsRef<OsStr>,
		I: IntoIterator<Item = S>,
		S: AsRef<OsStr>,
	{
		self.command(command, args)
			.run()
			.await
			.map_err(|e| RuntimeError::Internal(e.into()))
	}
}
