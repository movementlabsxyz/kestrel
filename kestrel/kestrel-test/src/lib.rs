//! This crate is a testing utility for kestrel.
//! It tests many of the macros in the kestrel crate.

#[cfg(test)]
mod tests {
	use kestrel::{abort, await_allow_abort, end};
	use tokio::time::sleep;
	use tokio::time::Duration;

	#[tokio::test]
	async fn test_await_allow_abort() -> Result<(), anyhow::Error> {
		let task1 = kestrel::task(async {
			sleep(Duration::from_millis(100)).await;
			Ok::<_, ()>(1)
		});

		let task2 = kestrel::task(async {
			sleep(Duration::from_millis(200)).await;
			Ok::<_, ()>("hello")
		});

		let result = await_allow_abort!(task1, task2);
		assert!(result.is_ok());

		Ok(())
	}

	#[tokio::test]
	async fn test_abort() {
		let task1 = kestrel::task(async {
			sleep(Duration::from_secs(1)).await;
			Ok::<_, ()>(1)
		});

		let task2 = kestrel::task(async {
			sleep(Duration::from_secs(1)).await;
			Ok::<_, ()>("hello")
		});

		abort!(task1, task2);

		assert!(task1.is_aborted());
		assert!(task2.is_aborted());
	}

	#[tokio::test]
	async fn test_end() {
		let task1 = kestrel::task(async {
			sleep(Duration::from_millis(100)).await;
			Ok::<_, ()>(1)
		});

		let task2 = kestrel::task(async {
			sleep(Duration::from_millis(200)).await;
			Ok::<_, ()>("hello")
		});

		let result = end!(task1, task2);
		assert!(result.is_ok());
	}

	#[tokio::test]
	async fn test_mixed_task_types() {
		let task1 = kestrel::task(async {
			sleep(Duration::from_millis(100)).await;
			Ok::<_, ()>(1)
		});

		let task2 = kestrel::task(async {
			sleep(Duration::from_millis(200)).await;
			Ok::<_, ()>("hello")
		});

		let task3 = kestrel::task(async {
			sleep(Duration::from_millis(300)).await;
			Ok::<_, ()>(true)
		});

		let result = await_allow_abort!(task1, task2, task3);
		assert!(result.is_ok());
	}
}
