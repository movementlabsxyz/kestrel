use crate::process::{Pipe, ProcessError, ProcessOperations};
use commander::Command as InnerCommand;
use std::ffi::OsStr;
use std::path::Path;
use tokio::sync::mpsc::Sender;

/// Runs a command on the command line and captures its output.
pub struct Command {
	inner: InnerCommand,
}

impl Command {
	/// Create a new Command instance from an inner command.
	pub fn new(command: InnerCommand) -> Self {
		Self { inner: command }
	}

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
			inner: InnerCommand::line(
				command,
				args,
				working_dir,
				capture_output,
				stdout_senders,
				stderr_senders,
			),
		}
	}

	/// Sets whether to capture the output of the command.
	pub fn set_capture_output(&mut self, capture_output: bool) -> &mut Self {
		self.inner.set_capture_output(capture_output);
		self
	}

	/// Appends a sender for the standard output of the command.
	pub fn append_stdout(&mut self, sender: Sender<String>) -> &mut Self {
		self.inner.append_stdout(sender);
		self
	}

	/// Appends a sender for the standard error of the command.
	pub fn append_stderr(&mut self, sender: Sender<String>) -> &mut Self {
		self.inner.append_stderr(sender);
		self
	}

	/// Working directory of the command.
	pub fn get_current_dir(&self) -> Option<&Path> {
		self.inner.get_current_dir()
	}
}

impl ProcessOperations for Command {
	fn run(mut self) -> impl std::future::Future<Output = Result<String, ProcessError>> + Send {
		async move { self.inner.run().await.map_err(|e| ProcessError::Runtime(e.into())) }
	}

	fn pipe(
		&mut self,
		pipe: Pipe,
		sender: tokio::sync::mpsc::Sender<String>,
	) -> Result<(), ProcessError> {
		match pipe {
			Pipe::STDOUT => {
				self.append_stdout(sender);
				Ok(())
			}
			Pipe::STDERR => {
				self.append_stderr(sender);
				Ok(())
			}
		}
	}
}
