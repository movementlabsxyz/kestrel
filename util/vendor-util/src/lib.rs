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

/// Gets the path to a given vendor directory based on the strategy
pub fn vendor_path(
	vendor_name: impl AsRef<str>,
	strategy: &VendorStrategy,
) -> Result<std::path::PathBuf, VendorUtilError> {
	match strategy {
		VendorStrategy::DotVendor => {
			let vendors_path = vendors_path()?;
			Ok(vendors_path.join(vendor_name.as_ref()))
		}
		VendorStrategy::TargetVendor => {
			let workspace_root = workspace_root()?;
			Ok(workspace_root
				.join("target")
				.join("release")
				.join("vendor")
				.join(vendor_name.as_ref())
				.join("revision"))
		}
	}
}

/// The strategt to use when vendoring
#[derive(Debug, Clone)]
pub enum VendorStrategy {
	/// Vendors into .vendor/vendor_name
	DotVendor,
	/// Vendors into target/release/vendor_name/revision
	TargetVendor,
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
	/// The strategy to use when vendoring.
	pub strategy: VendorStrategy,
}

impl VendorPlan {
	/// Creates a new [VendorPlan] with the specified strategy
	pub fn new(
		vendor_name: String,
		git_rev: String,
		git_url: String,
		strategy: VendorStrategy,
	) -> Self {
		Self { vendor_name, git_rev, git_url, strategy }
	}

	/// Creates a new [VendorPlan] with the default DotVendor strategy
	pub fn new_dot_vendor(vendor_name: String, git_rev: String, git_url: String) -> Self {
		Self::new(vendor_name, git_rev, git_url, VendorStrategy::DotVendor)
	}

	/// Creates a new [VendorPlan] with the TargetVendor strategy
	pub fn new_target_vendor(vendor_name: String, git_rev: String, git_url: String) -> Self {
		Self::new(vendor_name, git_rev, git_url, VendorStrategy::TargetVendor)
	}

	/// Renames the vendor plan to a new name
	pub fn rename(&mut self, new_name: String) {
		self.vendor_name = new_name;
	}

	/// Sets the strategy of the vendor plan
	pub fn set_strategy(&mut self, strategy: VendorStrategy) {
		self.strategy = strategy;
	}

	/// Execute the vendor plan, cloning or updating the repository as needed.
	/// Returns a Vendor instance if successful.
	pub fn execute(&self) -> Result<Vendor, VendorPlanError> {
		let vendor_path =
			vendor_path(&self.vendor_name, &self.strategy).context("Failed to get vendor path")?;

		// For TargetVendor strategy, we always want a fresh clone
		let needs_clone = match self.strategy {
			VendorStrategy::TargetVendor => {
				// Remove existing directory if it exists
				if vendor_path.exists() {
					std::fs::remove_dir_all(&vendor_path).map_err(VendorPlanError::RemoveDir)?;
				}
				// Create parent directories
				std::fs::create_dir_all(vendor_path.parent().unwrap())
					.map_err(VendorPlanError::CreateDir)?;
				true
			}
			VendorStrategy::DotVendor => {
				if vendor_path.exists() {
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
							std::fs::remove_dir_all(&vendor_path)
								.map_err(VendorPlanError::RemoveDir)?;
							true
						}
					}
				} else {
					// Directory doesn't exist, need to clone
					std::fs::create_dir_all(vendor_path.parent().unwrap())
						.map_err(VendorPlanError::CreateDir)?;
					true
				}
			}
		};

		if needs_clone {
			// Clone the repository
			let repo = git2::Repository::clone(&self.git_url, &vendor_path)?;

			// Fetch and checkout the specific revision
			let rev = repo.revparse_single(&self.git_rev)?;
			repo.checkout_tree(&rev, None)?;
			repo.set_head_detached(rev.id())?;
		} else {
			// Update existing repository (only for DotVendor strategy)
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

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_vendors_dot_vendor() -> Result<(), anyhow::Error> {
		// remove the .vendor directory if it exists
		let vendors_path = vendors_path()?;
		if vendors_path.exists() {
			std::fs::remove_dir_all(vendors_path)?;
		}

		// create a new vendor plan with DotVendor strategy
		let plan = VendorPlan::try_from_cargo_dep("qip", VendorStrategy::DotVendor)?;
		let vendor = plan.execute()?;

		// check that qip is in the vendor path and is checked out at the correct hash
		let qip_path = vendor.path;
		assert!(qip_path.exists());
		let qip_git = git2::Repository::open(qip_path)?;
		let qip_head = qip_git.head()?;
		let qip_head_id = qip_head.target().unwrap();
		let qip_head_id = qip_head_id.to_string();
		assert_eq!(qip_head_id, "070d5bcd1b248673d89faddae3a19f7894ab357e");

		Ok(())
	}

	#[test]
	fn test_vendors_target_vendor() -> Result<(), anyhow::Error> {
		// create a new vendor plan with TargetVendor strategy
		let plan = VendorPlan::try_from_cargo_dep("qip", VendorStrategy::TargetVendor)?;
		let vendor = plan.execute()?;

		// check that qip is in the target path and is checked out at the correct hash
		let qip_path = vendor.path;
		assert!(qip_path.exists());
		let qip_git = git2::Repository::open(qip_path)?;
		let qip_head = qip_git.head()?;
		let qip_head_id = qip_head.target().unwrap();
		let qip_head_id = qip_head_id.to_string();
		assert_eq!(qip_head_id, "070d5bcd1b248673d89faddae3a19f7894ab357e");

		Ok(())
	}
}
