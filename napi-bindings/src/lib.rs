//! # Sockudo WebSocket - Ultra-fast Node.js bindings
//!
//! High-performance WebSocket bindings using NAPI-RS v3 with lock-free data structures.
//!
//! ## Features
//!
//! - **Lock-free message queues** for zero-contention concurrent access
//! - **Configurable Tokio runtime** with adjustable worker threads
//! - **Zero-copy message handling** using shared buffers
//! - **SIMD-accelerated** frame masking and UTF-8 validation
//! - **Backpressure support** for high-throughput scenarios
//! - **Integrated pub/sub** for topic-based messaging

#![allow(clippy::new_without_default)]

mod client;
mod config;
mod error;
mod message;
mod pubsub;
mod runtime;
mod server;
mod stream;

pub use client::*;
pub use config::*;
pub use error::*;
pub use message::*;
pub use pubsub::*;
pub use runtime::*;
pub use server::*;
pub use stream::*;
