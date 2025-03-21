use anyhow::Result;
use futures::future::try_join;
use std::ffi::OsStr;
use std::path::Path;
use std::process::Stdio;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::Command as InnerCommand;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::mpsc::Sender;
use tracing::info;

/// Pipes output to stdout/stderr and broadcasts it via multiple channels.
async fn pipe_output<R, O>(
	reader: R,
	mut default_writer: BufWriter<O>, // Default stdout/stderr
	senders: &Vec<Sender<String>>,    // Multiple fanout receivers
	capture_output: bool,
	mut output: Option<&mut String>, // Optional in-memory capture
) -> Result<()>
where
	R: tokio::io::AsyncRead + Unpin + Send + 'static,
	O: tokio::io::AsyncWrite + Unpin + Send + 'static,
{
	let mut reader = BufReader::new(reader).lines();
	while let Ok(Some(line)) = reader.next_line().await {
		let formatted_line = format!("{}\n", line);
		let line_bytes = formatted_line.as_bytes();

		// Write to default stdout/stderr
		default_writer.write_all(line_bytes).await?;
		default_writer.flush().await?;

		// Fan out to all senders (non-blocking)
		for sender in senders {
			let _ = sender.send(formatted_line.clone()).await; // Clone per receiver
		}

		// Capture in memory if needed
		if capture_output {
			if let Some(ref mut output) = output {
				output.push_str(&formatted_line);
			}
		}
	}
	Ok(())
}

/// Runs a command with full stdout/stderr fanout.
pub async fn run_command_with_fanout<C, I, S>(
	command: C,
	args: I,
	working_dir: Option<&Path>,
	capture_output: bool,
	stdout_senders: Vec<Sender<String>>, // Multiple fanout receivers
	stderr_senders: Vec<Sender<String>>,
) -> Result<String>
where
	C: AsRef<OsStr> + Send,
	I: IntoIterator<Item = S> + Send,
	S: AsRef<OsStr>,
{
	let mut command = Command::new(command, capture_output, stdout_senders, stderr_senders);
	command.args(args);
	if let Some(dir) = working_dir {
		command.current_dir(dir);
	}
	command.run().await
}

/// Builder for running commands
pub struct Command {
	inner: InnerCommand,
	capture_output: bool,
	stdout_senders: Vec<Sender<String>>,
	stderr_senders: Vec<Sender<String>>,
}

impl Command {
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
		let mut command = Command::new(command, capture_output, stdout_senders, stderr_senders);
		command.args(args);
		if let Some(dir) = working_dir {
			command.current_dir(dir);
		}
		command
	}

	pub fn new(
		program: impl AsRef<OsStr>,
		capture_output: bool,
		stdout_senders: Vec<Sender<String>>,
		stderr_senders: Vec<Sender<String>>,
	) -> Self {
		let inner = InnerCommand::new(program);
		Self { inner, capture_output, stdout_senders, stderr_senders }
	}

	pub fn arg<S>(&mut self, arg: S) -> &mut Self
	where
		S: AsRef<OsStr>,
	{
		self.inner.arg(arg);
		self
	}

	pub fn args<I, S>(&mut self, args: I) -> &mut Self
	where
		I: IntoIterator<Item = S>,
		S: AsRef<OsStr>,
	{
		self.inner.args(args);
		self
	}

	pub fn append_stdout(&mut self, sender: Sender<String>) -> &mut Self {
		self.stdout_senders.push(sender);
		self
	}

	pub fn append_stderr(&mut self, sender: Sender<String>) -> &mut Self {
		self.stderr_senders.push(sender);
		self
	}

	pub fn current_dir<P: AsRef<Path>>(&mut self, dir: P) -> &mut Self {
		self.inner.current_dir(dir);
		self
	}

	/// Runs the command and captures its output while streaming it.
	pub async fn run(&mut self) -> Result<String> {
		let cmd_display = self.inner.as_std().get_program().to_string_lossy().into_owned();
		let args_display = self
			.inner
			.as_std()
			.get_args()
			.map(|s| s.to_string_lossy())
			.collect::<Vec<_>>()
			.join(" ");
		let working_dir = self
			.inner
			.as_std()
			.get_current_dir()
			.map(|p| p.to_string_lossy().into_owned())
			.unwrap_or_else(|| "default".to_string());

		info!("Running command: {cmd_display} {args_display} in {working_dir}");

		// Signal handling
		let (tx, rx) = tokio::sync::oneshot::channel();

		let mut sigterm = signal(SignalKind::terminate())?;
		let mut sigint = signal(SignalKind::interrupt())?;
		let mut sigquit = signal(SignalKind::quit())?;

		tokio::spawn(async move {
			tokio::select! {
				_ = sigterm.recv() => { let _ = tx.send(()); }
				_ = sigint.recv() => { let _ = tx.send(()); }
				_ = sigquit.recv() => { let _ = tx.send(()); }
			}
		});

		let mut child = self.inner.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;

		let stdout = child.stdout.take().ok_or_else(|| {
			anyhow::anyhow!("Failed to capture standard output from command {cmd_display}")
		})?;
		let stderr = child.stderr.take().ok_or_else(|| {
			anyhow::anyhow!("Failed to capture standard error from command {cmd_display}")
		})?;

		let mut stdout_output = if self.capture_output { Some(String::new()) } else { None };
		let mut stderr_output = if self.capture_output { Some(String::new()) } else { None };

		let stdout_writer = BufWriter::new(io::stdout());
		let stderr_writer = BufWriter::new(io::stderr());

		let stdout_future = pipe_output(
			stdout,
			stdout_writer,
			&self.stdout_senders,
			self.capture_output,
			stdout_output.as_mut(),
		);
		let stderr_future = pipe_output(
			stderr,
			stderr_writer,
			&self.stderr_senders,
			self.capture_output,
			stderr_output.as_mut(),
		);

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
				stderr_output.unwrap_or_else(|| "Unknown error".to_string())
			));
		}

		Ok(stdout_output.unwrap_or_default())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use anyhow::Result;
	use tokio::sync::mpsc;

	/// Test running a simple command and capturing its output.
	#[tokio::test]
	async fn test_run_command_with_capture() -> Result<()> {
		let output =
			run_command_with_fanout("echo", &["Hello, world!"], None, true, vec![], vec![]).await?;

		assert_eq!(output, "Hello, world!\n");
		Ok(())
	}

	/// Test running a command with multiple subscribers listening via channels.
	#[tokio::test]
	async fn test_run_command_with_fanout_channels() -> Result<()> {
		// Create multiple stdout/stderr channels
		let (stdout_tx1, mut stdout_rx1) = mpsc::channel(10);
		let (stdout_tx2, mut stdout_rx2) = mpsc::channel(10);
		let (stderr_tx, mut stderr_rx) = mpsc::channel(10);

		// Clone senders for fanout
		let stdout_senders = vec![stdout_tx1.clone(), stdout_tx2.clone()];
		let stderr_senders = vec![stderr_tx.clone()];

		// Spawn the command with fanout channels
		let command_future = tokio::spawn(run_command_with_fanout(
			"sh",
			&["-c", "echo Hello && echo Error >&2"],
			None,
			true,
			stdout_senders,
			stderr_senders,
		));

		// Wait for messages to arrive naturally
		let stdout_output1 = stdout_rx1.recv().await;
		let stdout_output2 = stdout_rx2.recv().await;
		let stderr_output = stderr_rx.recv().await;

		// Ensure all values exist
		assert!(stdout_output1.is_some());
		assert!(stdout_output2.is_some());
		assert!(stderr_output.is_some());

		// Validate fanout behavior
		assert_eq!(stdout_output1.unwrap(), "Hello\n");
		assert_eq!(stdout_output2.unwrap(), "Hello\n");
		assert_eq!(stderr_output.unwrap(), "Error\n");

		// Ensure the command completes
		command_future.await??;
		Ok(())
	}

	/// Test that multiple subscribers receive identical output.
	#[tokio::test]
	async fn test_multiple_stdout_subscribers() -> Result<()> {
		let (stdout_tx1, mut stdout_rx1) = mpsc::channel(10);
		let (stdout_tx2, mut stdout_rx2) = mpsc::channel(10);

		let stdout_senders = vec![stdout_tx1, stdout_tx2];

		let command_future = tokio::spawn(run_command_with_fanout(
			"echo",
			&["Test Output"],
			None,
			true,
			stdout_senders,
			vec![],
		));

		// Wait for messages
		let stdout_output1 = stdout_rx1.recv().await;
		let stdout_output2 = stdout_rx2.recv().await;

		// Ensure both values exist
		assert!(stdout_output1.is_some());
		assert!(stdout_output2.is_some());

		// Both subscribers should receive the same data
		assert_eq!(stdout_output1.unwrap(), "Test Output\n");
		assert_eq!(stdout_output2.unwrap(), "Test Output\n");

		// Ensure the command completes
		command_future.await??;
		Ok(())
	}
}
