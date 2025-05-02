use ignore::WalkBuilder;
use std::env;
use std::fs::File;
use std::io::BufWriter;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use zip::{write::SimpleFileOptions, ZipWriter};

#[derive(Debug, thiserror::Error)]
pub enum BuildtimeError {
	#[error("internal error: {0}")]
	Internal(#[source] Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug, Clone)]
pub struct Buildtime {
	directory_path: PathBuf,
	name: String,
}

impl Buildtime {
	pub fn new(directory_path: PathBuf, name: String) -> Self {
		Self { directory_path, name }
	}

	pub fn build(&self) -> Result<(), BuildtimeError> {
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

		// create an ignore walker
		let walker = WalkBuilder::new(self.directory_path.clone())
			.git_ignore(true)
			.git_exclude(true)
			.hidden(false)
			.build();

		// Walk through the source directory recursively
		for entry in walker.filter_map(Result::ok) {
			let path = entry.path();
			let name = path.strip_prefix(&self.directory_path).unwrap().to_str().unwrap();

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
		println!("cargo:rerun-if-changed={}", self.directory_path.display());

		Ok(())
	}
}
