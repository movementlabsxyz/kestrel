pub mod cargo;

use anyhow::Context;

/// Error type for buildtime operations.
#[derive(Debug, thiserror::Error)]
pub enum VendorUtilError {
	#[error("Vendor util: Internal error: {0}")]
	Internal(#[from] anyhow::Error),
}

/// Gets the path to the workspace root using the cargo metadata.
pub fn workspace_root() -> Result<std::path::PathBuf, VendorUtilError> {
	let metadata = cargo_metadata::MetadataCommand::new()
		.exec()
		.context("Failed to get cargo metadata")
		.map_err(VendorUtilError::Internal)?;
	Ok(metadata.workspace_root.clone().into())
}

/// Gets the path to the `.vendor` directory (workspace_root/.vendor)
pub fn vendors_path() -> Result<std::path::PathBuf, VendorUtilError> {
	let workspace_root = workspace_root()?;
	Ok(workspace_root.join(".vendor"))
}

/// Gets the path to a given vendor directory (vendors_path/vendor_name)
pub fn vendor_path(vendor_name: impl AsRef<str>) -> Result<std::path::PathBuf, VendorUtilError> {
	let vendors_path = vendors_path()?;
	Ok(vendors_path.join(vendor_name.as_ref()))
}

/// Error thrown when operating on a vendor plan.
#[derive(Debug, thiserror::Error)]
pub enum VendorPlanError {
	#[error("Vendor Plan: Internal error: {0}")]
	Internal(#[from] anyhow::Error),
	#[error("Vendor Plan: Git error: {0}")]
	Git(#[from] git2::Error),
	#[error("Vendor Plan: Failed to create vendor directory: {0}")]
	CreateDir(std::io::Error),
	#[error("Vendor Plan: Failed to remove existing vendor directory: {0}")]
	RemoveDir(std::io::Error),
}

/// A vendor plan is a git repository that should be vendored into the workspace.
#[derive(Debug, Clone)]
pub struct VendorPlan {
	/// The name of the vendor.
	pub vendor_name: String,
	/// The git revision of the vendor.
	pub git_rev: String,
	/// The git url of the vendor.
	pub git_url: String,
}

impl VendorPlan {
	/// Creates a new [VendorPlan]
	pub fn new(vendor_name: String, git_rev: String, git_url: String) -> Self {
		Self { vendor_name, git_rev, git_url }
	}

	/// Execute the vendor plan, cloning or updating the repository as needed.
	/// Returns a Vendor instance if successful.
	pub fn execute(&self) -> Result<Vendor, VendorPlanError> {
		let vendor_path = vendor_path(&self.vendor_name).context("Failed to get vendor path")?;

		// If the directory exists, check if it's the same repository
		let needs_clone = if vendor_path.exists() {
			match git2::Repository::open(&vendor_path) {
				Ok(repo) => {
					// Check if remote URL matches
					if let Ok(remote) = repo.find_remote("origin") {
						if let Some(url) = remote.url() {
							if url != self.git_url {
								// Different repository, need to remove and clone
								std::fs::remove_dir_all(&vendor_path)
									.map_err(VendorPlanError::RemoveDir)?;
								true
							} else {
								// Same repository, just need to fetch and checkout
								false
							}
						} else {
							true
						}
					} else {
						true
					}
				}
				Err(_) => {
					// Not a git repository, remove and clone
					std::fs::remove_dir_all(&vendor_path).map_err(VendorPlanError::RemoveDir)?;
					true
				}
			}
		} else {
			// Directory doesn't exist, need to clone
			std::fs::create_dir_all(vendor_path.parent().unwrap())
				.map_err(VendorPlanError::CreateDir)?;
			true
		};

		if needs_clone {
			// Clone the repository
			let repo = git2::Repository::clone(&self.git_url, &vendor_path)?;

			// Fetch and checkout the specific revision
			let rev = repo.revparse_single(&self.git_rev)?;
			repo.checkout_tree(&rev, None)?;
			repo.set_head_detached(rev.id())?;
		} else {
			// Update existing repository
			let repo = git2::Repository::open(&vendor_path)?;

			// Fetch updates
			let mut remote = repo.find_remote("origin")?;
			remote.fetch(&[&self.git_rev], None, None)?;

			// Checkout the specific revision
			let rev = repo.revparse_single(&self.git_rev)?;
			repo.checkout_tree(&rev, None)?;
			repo.set_head_detached(rev.id())?;
		}

		Ok(Vendor { plan: self.clone(), path: vendor_path })
	}
}

/// Error thrown when operating on a vendor.
#[derive(Debug, thiserror::Error)]
pub enum VendorError {
	#[error("Vendor: Internal error: {0}")]
	Internal(#[from] anyhow::Error),
}

/// A vendor is a git repository that is vendored into the workspace.
///
/// Mainly, this marks successful vendoring.
#[derive(Debug, Clone)]
pub struct Vendor {
	/// The plan of the vendor.
	pub plan: VendorPlan,
	/// The path to the vendor.
	pub path: std::path::PathBuf,
}
