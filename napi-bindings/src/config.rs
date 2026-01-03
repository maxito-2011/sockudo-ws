//! WebSocket configuration options

use napi_derive::napi;
use serde::{Deserialize, Serialize};

/// Compression mode for WebSocket connections
#[napi]
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum Compression {
    /// No compression
    #[default]
    Disabled,
    /// Dedicated compressor per connection (more memory, better ratio)
    Dedicated,
    /// Shared compressor across connections (less memory)
    Shared,
    /// Shared with 4KB sliding window
    Shared4KB,
    /// Shared with 8KB sliding window
    Shared8KB,
    /// Shared with 16KB sliding window
    Shared16KB,
}

impl From<Compression> for sockudo_ws::Compression {
    fn from(c: Compression) -> Self {
        match c {
            Compression::Disabled => sockudo_ws::Compression::Disabled,
            Compression::Dedicated => sockudo_ws::Compression::Dedicated,
            Compression::Shared => sockudo_ws::Compression::Shared,
            Compression::Shared4KB => sockudo_ws::Compression::Shared4KB,
            Compression::Shared8KB => sockudo_ws::Compression::Shared8KB,
            Compression::Shared16KB => sockudo_ws::Compression::Shared16KB,
        }
    }
}

/// WebSocket server/client configuration
///
/// @example
/// ```javascript
/// const config = new Config({
///   maxMessageSize: 16 * 1024 * 1024,  // 16MB
///   maxFrameSize: 1 * 1024 * 1024,     // 1MB
///   compression: Compression.Shared,
///   idleTimeout: 60,
///   autoPing: true,
///   pingInterval: 30
/// });
/// ```
#[napi(object)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Maximum message size in bytes (default: 64MB)
    #[serde(default = "default_max_message_size")]
    pub max_message_size: Option<u32>,

    /// Maximum frame size in bytes (default: 16MB)
    #[serde(default)]
    pub max_frame_size: Option<u32>,

    /// Write buffer size in bytes (default: 16KB)
    #[serde(default)]
    pub write_buffer_size: Option<u32>,

    /// Compression mode (default: Disabled)
    #[serde(default)]
    pub compression: Option<Compression>,

    /// Idle timeout in seconds (default: 120, 0 = disabled)
    #[serde(default)]
    pub idle_timeout: Option<u32>,

    /// Maximum backpressure in bytes (default: 1MB)
    #[serde(default)]
    pub max_backpressure: Option<u32>,

    /// Enable automatic ping/pong (default: true)
    #[serde(default)]
    pub auto_ping: Option<bool>,

    /// Ping interval in seconds (default: 30)
    #[serde(default)]
    pub ping_interval: Option<u32>,

    /// High water mark for backpressure (default: 64KB)
    #[serde(default)]
    pub high_water_mark: Option<u32>,

    /// Low water mark for backpressure (default: 16KB)
    #[serde(default)]
    pub low_water_mark: Option<u32>,
}

fn default_max_message_size() -> Option<u32> {
    Some(64 * 1024 * 1024)
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_message_size: Some(64 * 1024 * 1024),
            max_frame_size: Some(16 * 1024 * 1024),
            write_buffer_size: Some(16 * 1024),
            compression: Some(Compression::Disabled),
            idle_timeout: Some(120),
            max_backpressure: Some(1024 * 1024),
            auto_ping: Some(true),
            ping_interval: Some(30),
            high_water_mark: Some(64 * 1024),
            low_water_mark: Some(16 * 1024),
        }
    }
}

impl From<Config> for sockudo_ws::Config {
    fn from(c: Config) -> Self {
        let mut builder = sockudo_ws::Config::builder();

        if let Some(size) = c.max_message_size {
            builder = builder.max_message_size(size as usize);
        }
        if let Some(size) = c.max_frame_size {
            builder = builder.max_frame_size(size as usize);
        }
        if let Some(size) = c.write_buffer_size {
            builder = builder.write_buffer_size(size as usize);
        }
        if let Some(compression) = c.compression {
            builder = builder.compression(compression.into());
        }
        if let Some(timeout) = c.idle_timeout {
            builder = builder.idle_timeout(timeout);
        }
        if let Some(backpressure) = c.max_backpressure {
            builder = builder.max_backpressure(backpressure as usize);
        }
        if let Some(auto_ping) = c.auto_ping {
            builder = builder.auto_ping(auto_ping);
        }
        if let Some(interval) = c.ping_interval {
            builder = builder.ping_interval(interval);
        }

        builder.build()
    }
}

/// Create a configuration optimized for high-frequency trading
///
/// - Minimal latency settings
/// - Compression disabled
/// - Aggressive timeouts
///
/// @returns HFT-optimized configuration
#[napi]
pub fn hft_config() -> Config {
    Config {
        max_message_size: Some(64 * 1024), // 64KB - small messages
        max_frame_size: Some(64 * 1024),
        write_buffer_size: Some(16 * 1024),
        compression: Some(Compression::Disabled), // No compression overhead
        idle_timeout: Some(5),                    // Aggressive timeout
        max_backpressure: Some(256 * 1024),
        auto_ping: Some(true),
        ping_interval: Some(5), // Frequent pings for connection health
        high_water_mark: Some(32 * 1024),
        low_water_mark: Some(8 * 1024),
    }
}

/// Create a configuration optimized for high throughput
///
/// - Large buffers
/// - Compression enabled
/// - Relaxed timeouts
///
/// @returns Throughput-optimized configuration
#[napi]
pub fn throughput_config() -> Config {
    Config {
        max_message_size: Some(128 * 1024 * 1024), // 128MB
        max_frame_size: Some(16 * 1024 * 1024),    // 16MB
        write_buffer_size: Some(64 * 1024),        // 64KB buffer
        compression: Some(Compression::Shared),
        idle_timeout: Some(300), // 5 minutes
        max_backpressure: Some(16 * 1024 * 1024),
        auto_ping: Some(true),
        ping_interval: Some(60),
        high_water_mark: Some(256 * 1024),
        low_water_mark: Some(64 * 1024),
    }
}

/// Create a uWebSockets-compatible configuration
///
/// Matches uWebSockets default settings for easy migration.
///
/// @returns uWebSockets-compatible configuration
#[napi]
pub fn uws_config() -> Config {
    Config {
        max_message_size: Some(16 * 1024),
        max_frame_size: Some(16 * 1024),
        write_buffer_size: Some(16 * 1024),
        compression: Some(Compression::Shared),
        idle_timeout: Some(10),
        max_backpressure: Some(1024 * 1024),
        auto_ping: Some(true),
        ping_interval: Some(30),
        high_water_mark: Some(64 * 1024),
        low_water_mark: Some(16 * 1024),
    }
}
