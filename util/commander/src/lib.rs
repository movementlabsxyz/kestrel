use anyhow::Result;
use futures::future::try_join;
use itertools::Itertools;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command as InnerCommand;
use tokio::signal::unix::{signal, SignalKind};
use tracing::info;

use std::ffi::OsStr;
use std::path::Path;
use std::process::Stdio;

async fn pipe_output<R: tokio::io::AsyncRead + Unpin + Send + 'static>(
	reader: R,
	mut writer: io::Stdout,
	output: &mut String,
) -> Result<()> {
	let mut reader = BufReader::new(reader).lines();
	while let Ok(Some(line)) = reader.next_line().await {
		writer.write_all(line.as_bytes()).await?;
		writer.write_all(b"\n").await?;
		output.push_str(&line);
		output.push('\n');
	}
	Ok(())
}

async fn pipe_error_output<R: tokio::io::AsyncRead + Unpin + Send + 'static>(
	reader: R,
	mut writer: io::Stderr,
	output: &mut String,
) -> Result<()> {
	let mut reader = BufReader::new(reader).lines();
	while let Ok(Some(line)) = reader.next_line().await {
		writer.write_all(line.as_bytes()).await?;
		writer.write_all(b"\n").await?;
		output.push_str(&line);
		output.push('\n');
	}
	Ok(())
}

/// Runs a command, optionally setting the working directory, and pipes its output to stdout and stderr.
pub async fn run_command<C, I, S>(command: C, args: I, working_dir: Option<&Path>) -> Result<String>
where
	C: AsRef<OsStr>,
	I: IntoIterator<Item = S>,
	S: AsRef<OsStr>,
{
	let mut command = Command::new(command);
	command.args(args);
	if let Some(dir) = working_dir {
		command.current_dir(dir);
	}
	command.run_and_capture_output().await
}

/// Builder for running commands
pub struct Command(InnerCommand);

impl Command {
	pub fn new(program: impl AsRef<OsStr>) -> Self {
		let inner = InnerCommand::new(program);
		Self(inner)
	}

	pub fn arg<S>(&mut self, arg: S) -> &mut Self
	where
		S: AsRef<OsStr>,
	{
		self.0.arg(arg);
		self
	}

	pub fn args<I, S>(&mut self, args: I) -> &mut Self
	where
		I: IntoIterator<Item = S>,
		S: AsRef<OsStr>,
	{
		self.0.args(args);
		self
	}

	/// Sets the working directory for the command.
	pub fn current_dir<P: AsRef<Path>>(&mut self, dir: P) -> &mut Self {
		self.0.current_dir(dir);
		self
	}

	pub async fn run_and_capture_output(&mut self) -> Result<String> {
		let cmd_display = self.0.as_std().get_program().to_string_lossy().into_owned();
		let args_display = self.0.as_std().get_args().map(|s| s.to_string_lossy()).join(" ");
		let working_dir = self
			.0
			.as_std()
			.get_current_dir()
			.map(|p| p.to_string_lossy().into_owned())
			.unwrap_or_else(|| "default".to_string());

		info!("Running command: {cmd_display} {args_display} in {working_dir}");

		// Setup signal handling to terminate the child process
		let (tx, rx) = tokio::sync::oneshot::channel();

		let mut sigterm = signal(SignalKind::terminate())?;
		let mut sigint = signal(SignalKind::interrupt())?;
		let mut sigquit = signal(SignalKind::quit())?;

		tokio::spawn(async move {
			tokio::select! {
				_ = sigterm.recv() => {
					let _ = tx.send(());
				}
				_ = sigint.recv() => {
					let _ = tx.send(());
				}
				_ = sigquit.recv() => {
					let _ = tx.send(());
				}
			}
		});

		let mut child = self.0.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;

		let stdout = child.stdout.take().ok_or_else(|| {
			anyhow::anyhow!("Failed to capture standard output from command {cmd_display}")
		})?;
		let stderr = child.stderr.take().ok_or_else(|| {
			anyhow::anyhow!("Failed to capture standard error from command {cmd_display}")
		})?;

		let mut stdout_output = String::new();
		let mut stderr_output = String::new();

		let stdout_writer = io::stdout();
		let stderr_writer = io::stderr();

		let stdout_future = pipe_output(stdout, stdout_writer, &mut stdout_output);
		let stderr_future = pipe_error_output(stderr, stderr_writer, &mut stderr_output);

		let combined_future = try_join(stdout_future, stderr_future);

		tokio::select! {
			output = combined_future => {
				output?;
			}
			_ = rx => {
				let _ = child.kill().await;
				return Err(anyhow::anyhow!("Command {cmd_display} was terminated by signal"));
			}
		}

		let status = child.wait().await?;
		if !status.success() {
			return Err(anyhow::anyhow!(
				"Command {cmd_display} failed with args {args_display}\nError Output: {}",
				stderr_output
			));
		}

		Ok(stdout_output)
	}
}

#[cfg(test)]
pub mod tests {

	use super::*;

	#[tokio::test]
	async fn test_run_command() -> Result<(), anyhow::Error> {
		let output = run_command("echo", &["Hello, world!"], None).await?;
		assert_eq!(output, "Hello, world!\n");
		Ok(())
	}

	#[tokio::test]
	async fn test_run_command_with_working_dir() -> Result<(), anyhow::Error> {
		let temp_dir = tempfile::tempdir()?;
		let args: Vec<&str> = vec![];
		let output = run_command("pwd", args, Some(temp_dir.path())).await?;

		let output_trimmed = output.trim();
		let expected_path = temp_dir.path().to_str().unwrap();

		// Handle cases where macOS prepends `/private`
		if cfg!(target_os = "macos") {
			let private_prefixed_path = format!("/private{}", expected_path);
			assert!(
				output_trimmed == expected_path || output_trimmed == private_prefixed_path,
				"Expected '{}' or '{}', but got '{}'",
				expected_path,
				private_prefixed_path,
				output_trimmed
			);
		} else {
			assert_eq!(output_trimmed, expected_path);
		}

		Ok(())
	}
}
