use crate::VendorPlan;
use cargo_metadata::MetadataCommand;

/// Error thrown when operating on a vendor plan.
#[derive(Debug, thiserror::Error)]
pub enum CargoVendorPlanError {
	#[error("Cargo Vendor Plan: Internal error: {0}")]
	Internal(#[from] anyhow::Error),
	#[error("Cargo Vendor Plan: Dependency '{0}' not found in workspace")]
	DependencyNotFound(String),
	#[error("Cargo Vendor Plan: Dependency '{0}' is not a git dependency")]
	NotGitDependency(String),
	#[error("Cargo Vendor Plan: Git dependency '{0}' has no URL")]
	NoGitUrl(String),
	#[error("Cargo Vendor Plan: Git dependency '{0}' has no revision")]
	NoGitRevision(String),
}

impl VendorPlan {
	/// Attempts to create a VendorPlan from a cargo dependency name.
	/// The dependency must be a git dependency with a URL and revision.
	///
	/// NOTE: dependency must be in the crate.
	pub fn try_from_cargo_dep(dep_name: impl AsRef<str>) -> Result<Self, CargoVendorPlanError> {
		let dep_name = dep_name.as_ref();

		// Get cargo metadata
		let metadata = MetadataCommand::new()
			.exec()
			.map_err(|e| anyhow::anyhow!("Failed to get cargo metadata: {}", e))?;

		// Search through all workspace packages
		let dep = metadata
			.workspace_packages()
			.iter()
			.find_map(|pkg| {
				// Get all dependencies (normal, dev, and build)
				pkg.dependencies
					.iter()
					.find(|d| d.name == dep_name)
					.map(|d| (pkg.name.clone(), d.clone()))
			})
			.ok_or_else(|| CargoVendorPlanError::DependencyNotFound(dep_name.to_string()))?;

		let (_pkg_name, dep) = dep;

		// Check if it's a git dependency
		let source = dep
			.source
			.as_ref()
			.ok_or_else(|| CargoVendorPlanError::NotGitDependency(dep_name.to_string()))?;

		// Extract git URL and revision
		let (git_url, git_rev) = if source.starts_with("git+") || source.contains("?rev=") {
			// Handle both formats:
			// 1. git+{url}?rev={rev}
			// 2. {url}?rev={rev}
			let url = source.strip_prefix("git+").unwrap_or(source);

			// Split on ?rev= to get URL and revision
			let (url, rev) = url
				.split_once("?rev=")
				.ok_or_else(|| CargoVendorPlanError::NoGitRevision(dep_name.to_string()))?;

			// Remove any other query parameters from the URL
			let url = url.split_once('?').map(|(url, _)| url).unwrap_or(url);

			(url.to_string(), rev.to_string())
		} else {
			return Err(CargoVendorPlanError::NotGitDependency(dep_name.to_string()));
		};

		if git_url.is_empty() {
			return Err(CargoVendorPlanError::NoGitUrl(dep_name.to_string()));
		}

		Ok(VendorPlan::new(dep_name.to_string(), git_rev, git_url))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_try_from_cargo_dep() -> Result<(), anyhow::Error> {
		let plan = VendorPlan::try_from_cargo_dep("qip")?;
		assert_eq!(plan.vendor_name, "qip");
		assert_eq!(plan.git_url, "https://github.com/Renmusxd/RustQIP.git");
		assert_eq!(plan.git_rev, "070d5bcd1b248673d89faddae3a19f7894ab357e");
		Ok(())
	}
}
