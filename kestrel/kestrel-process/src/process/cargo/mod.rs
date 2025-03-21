use crate::process::{command::Command, Pipe, ProcessError, ProcessOperations};
use std::ffi::OsStr;
use std::future::Future;
use std::path::Path;
use tokio::sync::mpsc::Sender;

/// This trait ensures that the binary is imported from somewhere within the workspace.
/// This has the neat side effect of ensuring that most of the binary is already built.
pub trait RegisteredBin {
	/// Returns the binary name based on Cargo package name.
	fn cargo_bin() -> &'static str {
		env!("CARGO_PKG_NAME")
	}

	/// Checks if the current binary is inside a Cargo workspace.
	fn is_in_cargo_workspace() -> bool {
		Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml").exists()
	}

	/// Determines whether the build mode is debug or release.
	fn debug_or_release() -> &'static str {
		if cfg!(debug_assertions) {
			"debug"
		} else {
			"release"
		}
	}

	/// Ensures the binary is built when inside a Cargo workspace.
	fn build() -> impl Future<Output = Result<(), ProcessError>> + Send {
		async move {
			if Self::is_in_cargo_workspace() {
				commander::Command::line(
					"cargo",
					vec![
						"build".to_string(),
						if Self::debug_or_release() == "release" {
							"--release".to_string()
						} else {
							"".to_string()
						},
					],
					None,
					false,
					vec![], // No stdout senders
					vec![], // No stderr senders
				)
				.run()
				.await
				.map_err(|e| ProcessError::Buildtime(e.into()))?;
			}
			Ok(())
		}
	}

	/// Returns the binary path, handling workspace and standalone cases.
	fn cargo_bin_path() -> String {
		if Self::is_in_cargo_workspace() {
			let target_dir = format!(
				"{}/target/{}/{}",
				env!("CARGO_MANIFEST_DIR"),
				Self::debug_or_release(),
				Self::cargo_bin()
			);
			target_dir
		} else {
			// Assume the binary is globally available in PATH
			Self::cargo_bin().to_string()
		}
	}
}

#[macro_export]
macro_rules! kestrelize {
	() => {
		// Helper: Convert kebab-case to PascalCase
		macro_rules! kebab_to_pascal {
			($name:expr) => {{
				let mut result = String::new();
				let mut capitalize = true;
				for ch in $name.chars() {
					if ch == '-' {
						capitalize = true;
					} else if capitalize {
						result.push(ch.to_ascii_uppercase());
						capitalize = false;
					} else {
						result.push(ch);
					}
				}
				result
			}};
		}

		const _PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");
		const _STRUCT_NAME: &str = kebab_to_pascal!(_PACKAGE_NAME);

		// Use `paste` for dynamic struct name
		paste::paste! {
			#[allow(non_camel_case_types)]
			pub struct [<_STRUCT_NAME>];

			impl $crate::RegisteredBin for [<_STRUCT_NAME>] {
				fn cargo_bin() -> &'static str {
					_PACKAGE_NAME
				}
			}
		}
	};
}

/// Runs a command on the command line and captures its output.
pub struct Bin<B>
where
	B: RegisteredBin,
{
	phantom: std::marker::PhantomData<B>,
	runtime: Command,
}

impl<B> Bin<B>
where
	B: RegisteredBin,
{
	/// Create a new Command instance from a command-line-like string.
	pub fn line<C, I, S>(
		command: C,
		args: I,
		working_dir: Option<&Path>,
		capture_output: bool,
		stdout_senders: Vec<Sender<String>>, // Multiple fanout receivers
		stderr_senders: Vec<Sender<String>>,
	) -> Self
	where
		C: AsRef<OsStr> + Send,
		I: IntoIterator<Item = S> + Send,
		S: AsRef<OsStr>,
	{
		Self {
			phantom: std::marker::PhantomData,
			runtime: Command::line(
				command,
				args,
				working_dir,
				capture_output,
				stdout_senders,
				stderr_senders,
			),
		}
	}
}

impl<B> ProcessOperations for Bin<B>
where
	B: RegisteredBin + Send + Sync + 'static,
{
	fn run(self) -> impl std::future::Future<Output = Result<String, ProcessError>> + Send {
		async move {
			B::build().await?;
			self.runtime.run().await.map_err(|e| ProcessError::Runtime(e.into()))
		}
	}

	fn pipe(
		&mut self,
		pipe: Pipe,
		sender: tokio::sync::mpsc::Sender<String>,
	) -> Result<(), ProcessError> {
		self.runtime.pipe(pipe, sender)
	}
}
