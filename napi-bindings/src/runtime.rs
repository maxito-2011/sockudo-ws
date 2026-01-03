//! Tokio runtime configuration with adjustable worker threads

use napi::bindgen_prelude::*;
use napi_derive::napi;
use once_cell::sync::OnceCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::runtime::Runtime;

static RUNTIME: OnceCell<Runtime> = OnceCell::new();
static WORKER_THREADS: AtomicUsize = AtomicUsize::new(0);

/// Initialize the Tokio runtime with a specific number of worker threads.
///
/// Call this before creating any WebSocket servers or clients.
/// If not called, the runtime will use all available CPU cores.
///
/// @param workerThreads - Number of worker threads (0 = use all cores)
///
/// @example
/// ```javascript
/// import { initRuntime } from '@sockudo/ws';
///
/// // Use 4 worker threads
/// initRuntime(4);
///
/// // Or use all available cores
/// initRuntime(0);
/// ```
#[napi]
pub fn init_runtime(worker_threads: Option<u32>) -> Result<()> {
    let threads = worker_threads.unwrap_or(0) as usize;
    WORKER_THREADS.store(threads, Ordering::SeqCst);

    RUNTIME.get_or_try_init(|| {
        let mut builder = tokio::runtime::Builder::new_multi_thread();

        if threads > 0 {
            builder.worker_threads(threads);
        }

        builder
            .enable_all()
            .thread_name("sockudo-ws-worker")
            .build()
            .map_err(|e| {
                Error::new(
                    Status::GenericFailure,
                    format!("Failed to create runtime: {}", e),
                )
            })
    })?;

    Ok(())
}

/// Get the number of worker threads configured for the runtime.
///
/// Returns 0 if using the default (all available cores).
#[napi]
pub fn get_worker_threads() -> u32 {
    WORKER_THREADS.load(Ordering::SeqCst) as u32
}

/// Get the number of available CPU cores.
#[napi]
pub fn get_available_cores() -> u32 {
    std::thread::available_parallelism()
        .map(|p| p.get() as u32)
        .unwrap_or(1)
}

pub(crate) fn get_runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        let threads = WORKER_THREADS.load(Ordering::SeqCst);
        let mut builder = tokio::runtime::Builder::new_multi_thread();

        if threads > 0 {
            builder.worker_threads(threads);
        }

        builder
            .enable_all()
            .thread_name("sockudo-ws-worker")
            .build()
            .expect("Failed to create Tokio runtime")
    })
}
