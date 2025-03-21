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
				self.inner.append_stdout(sender);
				Ok(())
			}
			Pipe::STDERR => {
				self.inner.append_stderr(sender);
				Ok(())
			}
		}
	}
}
