use std::ffi::OsStr;
use std::fs::File;
use std::io::Cursor;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use zip::read::ZipArchive;

#[derive(Debug)]
pub enum WorkspacePath {
	PathBuf(PathBuf),
	TempDir(TempDir),
}

impl WorkspacePath {
	pub fn get_path(&self) -> &Path {
		match self {
			WorkspacePath::PathBuf(path) => path.as_path(),
			WorkspacePath::TempDir(temp_dir) => temp_dir.path(),
		}
	}
}

#[derive(Debug)]
pub struct Workspace {
	pub contracts_zip: &'static [u8],
	pub workspace_path: WorkspacePath,
}

/// Used to manage a contract workspace
impl Workspace {
	/// Creates a new contract workspace.
	pub fn new(contracts_zip: &'static [u8], workspace_path: WorkspacePath) -> Self {
		Workspace { contracts_zip, workspace_path }
	}

	/// Creates a new temporary contract workspace.
	pub fn try_temp(contracts_zip: &'static [u8]) -> Result<Self, std::io::Error> {
		let temp_dir = TempDir::new()?;
		Ok(Workspace { contracts_zip, workspace_path: WorkspacePath::TempDir(temp_dir) })
	}

	/// Generates a new workspaces in .debug/{uid}
	pub fn try_debug(contracts_zip: &'static [u8]) -> Result<Self, std::io::Error> {
		let uid = uuid::Uuid::new_v4();
		let path = Path::new(".debug").join(uid.to_string());
		Ok(Workspace { contracts_zip, workspace_path: WorkspacePath::PathBuf(path) })
	}

	/// Generate a new workspace in ~/.debug/{uid}
	pub fn try_debug_home(contracts_zip: &'static [u8]) -> Result<Self, std::io::Error> {
		let uid = uuid::Uuid::new_v4();
		let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
		let path = Path::new(&home).join(".debug").join(uid.to_string());
		Ok(Workspace { contracts_zip, workspace_path: WorkspacePath::PathBuf(path) })
	}

	/// Gets the workspace path
	pub fn get_workspace_path(&self) -> &Path {
		self.workspace_path.get_path()
	}

	/// Unzips the contracts zip file to the provided path.
	pub fn prepare_directory(&self) -> Result<(), std::io::Error> {
		// Determine the output directory
		let output_dir = match &self.workspace_path {
			WorkspacePath::PathBuf(path) => path.clone(),
			WorkspacePath::TempDir(temp_dir) => temp_dir.path().to_path_buf(),
		};

		// Read the embedded ZIP archive
		let cursor = Cursor::new(self.contracts_zip);
		let mut archive = ZipArchive::new(cursor)?;

		// Extract each file in the ZIP archive
		for i in 0..archive.len() {
			let mut file = archive.by_index(i)?;
			let outpath = output_dir.join(file.name());

			if file.is_dir() {
				std::fs::create_dir_all(&outpath)?;
			} else {
				if let Some(parent) = outpath.parent() {
					std::fs::create_dir_all(parent)?;
				}
				let mut outfile = File::create(&outpath)?;
				std::io::copy(&mut file, &mut outfile)?;

				// Set Unix permissions from the zip file
				if let Some(mode) = file.unix_mode() {
					outfile.set_permissions(std::fs::Permissions::from_mode(mode))?;
				}
			}
		}

		Ok(())
	}

	/// Constructs a command to run in the workspace
	pub fn command<C, I, S>(&self, command: C, args: I) -> commander::Command
	where
		C: AsRef<OsStr>,
		I: IntoIterator<Item = S>,
		S: AsRef<OsStr>,
	{
		let mut command = commander::Command::new(command, true, vec![], vec![]);
		command.args(args).current_dir(self.get_workspace_path());
		command
	}

	/// Prepares the directory and returns a command for the prepared directory
	pub fn prepared_command<C, I, S>(
		&self,
		command: C,
		args: I,
	) -> Result<commander::Command, anyhow::Error>
	where
		C: AsRef<OsStr>,
		I: IntoIterator<Item = S>,
		S: AsRef<OsStr>,
	{
		self.prepare_directory()?;
		Ok(self.command(command, args))
	}

	/// Runs a command in the workspace
	pub async fn run_command<C, I, S>(&self, command: C, args: I) -> Result<String, anyhow::Error>
	where
		C: AsRef<OsStr>,
		I: IntoIterator<Item = S>,
		S: AsRef<OsStr>,
	{
		// Implementation of the run_command function
		self.command(command, args).run().await
	}

	/// Prepares the workspace directory and runs a command
	pub async fn run<C, I, S>(&self, command: C, args: I) -> Result<String, anyhow::Error>
	where
		C: AsRef<OsStr>,
		I: IntoIterator<Item = S>,
		S: AsRef<OsStr>,
	{
		self.prepare_directory()?;
		self.run_command(command, args).await
	}
}

// Create a macro that will create a bespoke workspace struct fixed to a given include-dir "name"
#[macro_export]
macro_rules! workspace {
	($struct_name:ident, $name:expr) => {
		pub const ZIP: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/", $name, ".zip"));

		#[derive(Debug)]
		pub struct $struct_name {
			workspace: include_dir::Workspace,
		}

		impl $struct_name {
			/// Creates a new workspace from a given workspace path
			pub fn new(workspace_path: include_dir::WorkspacePath) -> Self {
				Self { workspace: include_dir::Workspace::new(ZIP, workspace_path) }
			}

			/// Creates a new temporary workspace
			pub fn try_temp() -> Result<Self, std::io::Error> {
				let temp_dir = include_dir::TempDir::new()?;
				let workspace_path = include_dir::WorkspacePath::TempDir(temp_dir);
				Ok(Self::new(workspace_path))
			}

			/// Generates a new workspaces in .debug/{uid}
			pub fn try_debug() -> Result<Self, std::io::Error> {
				let uuid = include_dir::uuid::Uuid::new_v4();
				let workspace_path =
					include_dir::WorkspacePath::PathBuf(Path::new(".debug").join(uuid.to_string()));
				Ok(Self::new(workspace_path))
			}

			/// Generates a new workspace in ~/.debug/{uid}
			pub fn try_debug_home() -> Result<Self, std::io::Error> {
				let uuid = include_dir::uuid::Uuid::new_v4();
				let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
				let workspace_path = include_dir::WorkspacePath::PathBuf(
					Path::new(&home).join(".debug").join(uuid.to_string()),
				);
				Ok(Self::new(workspace_path))
			}

			/// Gets the workspace path
			pub fn get_workspace_path(&self) -> &std::path::Path {
				self.workspace.get_workspace_path()
			}

			/// Unzips the contracts zip file to the provided path.
			pub fn prepare_directory(&self) -> Result<(), std::io::Error> {
				self.workspace.prepare_directory()
			}

			/// Constructs a command to run in the workspace
			pub fn command<C, I, S>(&self, command: C, args: I) -> include_dir::commander::Command
			where
				C: AsRef<OsStr>,
				I: IntoIterator<Item = S>,
				S: AsRef<OsStr>,
			{
				self.workspace.command(command, args)
			}

			/// Prepares the directory and returns a command for the prepared directory
			pub fn prepared_command<C, I, S>(
				&self,
				command: C,
				args: I,
			) -> Result<include_dir::commander::Command, anyhow::Error>
			where
				C: AsRef<OsStr>,
				I: IntoIterator<Item = S>,
				S: AsRef<OsStr>,
			{
				self.workspace.prepared_command(command, args)
			}

			pub async fn run_command<C, I, S>(
				&self,
				command: C,
				args: I,
			) -> Result<String, anyhow::Error>
			where
				C: AsRef<std::ffi::OsStr>,
				I: IntoIterator<Item = S>,
				S: AsRef<std::ffi::OsStr>,
			{
				self.workspace.run_command(command, args).await
			}

			/// Prepares the workspace and runs a command
			pub async fn run<C, I, S>(&self, command: C, args: I) -> Result<String, anyhow::Error>
			where
				C: AsRef<std::ffi::OsStr>,
				I: IntoIterator<Item = S>,
				S: AsRef<std::ffi::OsStr>,
			{
				self.prepare_directory()?;
				self.run_command(command, args).await
			}
		}
	};
}
