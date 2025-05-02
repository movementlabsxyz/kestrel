use include_dir::{Workspace as IncludeDirWorkspace, WorkspacePath};

#[derive(Debug)]
pub struct Workspace {
	workspace: IncludeDirWorkspace,
}

impl Workspace {
	pub fn new(contracts_zip: &'static [u8], workspace_path: WorkspacePath) -> Self {
		Self { workspace: IncludeDirWorkspace::new(contracts_zip, workspace_path) }
	}

	pub fn try_temp(contracts_zip: &'static [u8]) -> Result<Self, std::io::Error> {
		Ok(Self { workspace: IncludeDirWorkspace::try_temp(contracts_zip)? })
	}

	pub fn get_workspace_path(&self) -> &std::path::Path {
		self.workspace.get_workspace_path()
	}

	pub fn prepare_directory(&self) -> Result<(), std::io::Error> {
		self.workspace.prepare_directory()
	}

	pub async fn run_command<C, I, S>(&self, command: C, args: I) -> Result<String, anyhow::Error>
	where
		C: AsRef<std::ffi::OsStr>,
		I: IntoIterator<Item = S>,
		S: AsRef<std::ffi::OsStr>,
	{
		self.workspace.run_command(command, args).await
	}

	pub async fn run<C, I, S>(&self, command: C, args: I) -> Result<String, anyhow::Error>
	where
		C: AsRef<std::ffi::OsStr>,
		I: IntoIterator<Item = S>,
		S: AsRef<std::ffi::OsStr>,
	{
		self.workspace.run(command, args).await
	}
}

// Create a macro that will create a bespoke workspace struct fixed to a given vendor name
#[macro_export]
macro_rules! vendor_workspace {
	($struct_name:ident, $name:expr) => {
		pub const ZIP: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/", $name, ".zip"));

		#[derive(Debug)]
		pub struct $struct_name {
			workspace: include_vendor::Workspace,
		}

		impl $struct_name {
			/// Creates a new workspace with the given workspace path.
			pub fn new(workspace_path: include_vendor::WorkspacePath) -> Self {
				Self { workspace: include_vendor::Workspace::new(ZIP, workspace_path) }
			}

			/// Creates a new workspace with a temporary directory.
			pub fn try_temp() -> Result<Self, std::io::Error> {
				let temp_dir = include_vendor::TempDir::new()?;
				let workspace_path = include_vendor::WorkspacePath::TempDir(temp_dir);
				Ok(Self::new(workspace_path))
			}

			/// Gets the workspace path.
			pub fn get_workspace_path(&self) -> &std::path::Path {
				self.workspace.get_workspace_path()
			}

			/// Prepares the workspace.
			pub fn prepare_directory(&self) -> Result<(), std::io::Error> {
				self.workspace.prepare_directory()
			}

			/// Runs the given command.
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

			/// Prepares the workspace and runs the given command.
			pub async fn run<C, I, S>(&self, command: C, args: I) -> Result<String, anyhow::Error>
			where
				C: AsRef<std::ffi::OsStr>,
				I: IntoIterator<Item = S>,
				S: AsRef<std::ffi::OsStr>,
			{
				self.workspace.run(command, args).await
			}
		}
	};
}
