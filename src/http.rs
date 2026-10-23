//! Shared lifecycle for cancellable HTTP work; callers own destination policies.
use anyhow::{Result, ensure};
use std::{
    future::Future,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

pub(crate) fn run<T>(
    cancelled: &AtomicBool,
    timeout: Duration,
    work: impl Future<Output = Result<T>>,
) -> Result<T> {
    ensure!(!cancelled.load(Ordering::Relaxed), "HTTP request cancelled");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let deadline = Instant::now() + timeout;
    let result = runtime.block_on(async {
        let mut work = std::pin::pin!(work);
        loop {
            ensure!(!cancelled.load(Ordering::Relaxed), "HTTP request cancelled");
            ensure!(Instant::now() < deadline, "HTTP request timed out");
            if let Ok(result) = tokio::time::timeout(Duration::from_millis(10), work.as_mut()).await
            {
                return result;
            }
        }
    });
    runtime.shutdown_timeout(Duration::from_millis(10));
    result
}

pub(crate) async fn read_bounded(mut response: reqwest::Response, limit: usize) -> Result<Vec<u8>> {
    ensure!(
        response
            .content_length()
            .is_none_or(|len| len <= limit as u64),
        "HTTP response exceeds {limit} bytes"
    );
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(reqwest::Error::without_url)?
    {
        ensure!(
            chunk.len() <= limit - bytes.len(),
            "HTTP response exceeds {limit} bytes"
        );
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_http_work_can_time_out_or_be_cancelled() {
        let cancelled = AtomicBool::new(false);
        let started = Instant::now();
        let error = run(
            &cancelled,
            Duration::from_millis(20),
            std::future::pending::<Result<()>>(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("timed out"));
        assert!(started.elapsed() < Duration::from_secs(1));
        std::thread::scope(|scope| {
            scope.spawn(|| {
                std::thread::sleep(Duration::from_millis(20));
                cancelled.store(true, Ordering::Relaxed);
            });
            let error = run(
                &cancelled,
                Duration::from_secs(2),
                std::future::pending::<Result<()>>(),
            )
            .unwrap_err();
            assert!(error.to_string().contains("cancelled"));
        });
    }
}
