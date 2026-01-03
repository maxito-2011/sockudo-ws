//! WebSocket server with integrated pub/sub support

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use napi::bindgen_prelude::*;
use napi::threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode};
use napi_derive::napi;
use parking_lot::RwLock;
use tokio::net::TcpListener;
use tokio::sync::broadcast;

use crate::config::Config;
use crate::message::Message;
use crate::runtime::get_runtime;
use crate::stream::{WebSocket, create_websocket_with_pubsub};

/// Connection information passed to the connection handler
#[napi(object)]
#[derive(Clone, Debug)]
pub struct ConnectionInfo {
    /// Client IP address
    pub remote_addr: String,
    /// Request path (e.g., "/ws")
    pub path: String,
    /// Negotiated WebSocket protocol (if any)
    pub protocol: Option<String>,
    /// Connection ID (unique per server)
    pub connection_id: u32,
}

/// Server statistics
#[napi(object)]
#[derive(Clone, Debug, Default)]
pub struct ServerStats {
    /// Total connections accepted
    pub total_connections: u32,
    /// Currently active connections
    pub active_connections: u32,
    /// Total topics in pub/sub system
    pub topic_count: u32,
    /// Total messages published via server.publish()
    pub messages_published: u32,
}

struct ServerStatsInner {
    total_connections: AtomicU64,
    active_connections: AtomicU64,
    messages_published: AtomicU64,
}

impl Default for ServerStatsInner {
    fn default() -> Self {
        Self {
            total_connections: AtomicU64::new(0),
            active_connections: AtomicU64::new(0),
            messages_published: AtomicU64::new(0),
        }
    }
}

/// Options for creating a WebSocket server
#[napi(object)]
#[derive(Clone, Debug)]
pub struct ServerOptions {
    /// Port to listen on
    pub port: u16,
    /// Host to bind to (default: "0.0.0.0")
    pub host: Option<String>,
    /// WebSocket configuration
    pub config: Option<Config>,
    /// Send queue size per connection (0 = unbounded, default: 1024)
    pub queue_size: Option<u32>,
}

/// High-performance WebSocket server with integrated pub/sub
///
/// Uses lock-free data structures and the Tokio runtime for maximum throughput.
/// Includes built-in pub/sub support - each WebSocket connection can subscribe
/// to topics and publish messages.
///
/// @example
/// ```javascript
/// import { WebSocketServer, Message, Compression } from '@sockudo/ws';
///
/// const server = new WebSocketServer({
///   port: 8080,
///   config: { compression: Compression.Shared }
/// });
///
/// await server.start((ws, info) => {
///   console.log(`New connection from ${info.remoteAddr}`);
///
///   // Subscribe to topics
///   ws.subscribe('chat');
///   ws.subscribe(`user/${info.connectionId}`);
///
///   ws.onMessage((msg) => {
///     // Broadcast to all subscribers except sender
///     ws.publish('chat', msg);
///   });
///
///   ws.onClose((reason) => {
///     console.log(`Connection closed: ${reason.code}`);
///   });
/// });
///
/// // Server-level publish (reaches ALL subscribers)
/// server.publish('chat', Message.text('Server announcement!'));
/// ```
#[napi]
pub struct WebSocketServer {
    config: sockudo_ws::Config,
    host: String,
    port: u16,
    running: Arc<AtomicBool>,
    shutdown_tx: Arc<RwLock<Option<broadcast::Sender<()>>>>,
    stats: Arc<ServerStatsInner>,
    queue_size: usize,
    pubsub: Arc<sockudo_ws::pubsub::PubSubState>,
}

#[napi]
impl WebSocketServer {
    /// Create a new WebSocket server
    ///
    /// @param options - Server options including port, host, and config
    ///
    /// @example
    /// ```javascript
    /// const server = new WebSocketServer({ port: 8080 });
    /// ```
    #[napi(constructor)]
    pub fn new(options: ServerOptions) -> Self {
        let config: sockudo_ws::Config = options.config.unwrap_or_default().into();

        Self {
            config,
            host: options.host.unwrap_or_else(|| "0.0.0.0".to_string()),
            port: options.port,
            running: Arc::new(AtomicBool::new(false)),
            shutdown_tx: Arc::new(RwLock::new(None)),
            stats: Arc::new(ServerStatsInner::default()),
            queue_size: options.queue_size.unwrap_or(1024) as usize,
            pubsub: Arc::new(sockudo_ws::pubsub::PubSubState::new()),
        }
    }

    /// Start the WebSocket server
    ///
    /// @param onConnection - Callback for new connections, receives WebSocket and ConnectionInfo
    ///
    /// @example
    /// ```javascript
    /// await server.start((ws, info) => {
    ///   ws.subscribe('chat');
    ///   ws.onMessage((msg) => ws.publish('chat', msg));
    /// });
    /// ```
    #[napi]
    pub async fn start(
        &self,
        #[napi(ts_arg_type = "(ws: WebSocket, info: ConnectionInfo) => void")]
        on_connection: ThreadsafeFunction<(WebSocket, ConnectionInfo)>,
    ) -> Result<()> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Err(Error::new(Status::GenericFailure, "Server already running"));
        }

        let addr = format!("{}:{}", self.host, self.port);
        let config = self.config.clone();
        let stats = self.stats.clone();
        let running = self.running.clone();
        let queue_size = self.queue_size;
        let pubsub = self.pubsub.clone();

        let (shutdown_tx, _) = broadcast::channel(1);
        *self.shutdown_tx.write() = Some(shutdown_tx.clone());

        let listener = get_runtime()
            .spawn(async move { TcpListener::bind(&addr).await })
            .await
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

        let running_clone = running.clone();

        get_runtime().spawn(async move {
            run_server(
                listener,
                config,
                stats,
                running_clone,
                shutdown_tx,
                on_connection,
                queue_size,
                pubsub,
            )
            .await;
        });

        Ok(())
    }

    /// Stop the WebSocket server
    ///
    /// Gracefully shuts down the server and closes all connections.
    #[napi]
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);

        if let Some(tx) = self.shutdown_tx.read().as_ref() {
            let _ = tx.send(());
        }
    }

    // ==================== Pub/Sub ====================

    /// Publish a message to all subscribers of a topic
    ///
    /// Unlike `ws.publish()`, this sends to ALL subscribers including any
    /// socket that might have originated the message.
    ///
    /// @param topic - Topic to publish to
    /// @param message - Message to publish
    /// @returns Number of subscribers that received the message
    ///
    /// @example
    /// ```javascript
    /// // Broadcast server announcement to all chat subscribers
    /// server.publish('chat', Message.text('Server restarting in 5 minutes'));
    /// ```
    #[napi]
    pub fn publish(&self, topic: String, message: &Message) -> u32 {
        let msg: sockudo_ws::Message = message.into();
        let result = self.pubsub.publish(&topic, msg);

        match result {
            sockudo_ws::pubsub::PublishResult::Published(count) => {
                self.stats
                    .messages_published
                    .fetch_add(count as u64, Ordering::Relaxed);
                count as u32
            }
            _ => 0,
        }
    }

    /// Publish a text message to a topic
    ///
    /// @param topic - Topic to publish to
    /// @param text - Text to publish
    /// @returns Number of subscribers
    #[napi]
    pub fn publish_text(&self, topic: String, text: String) -> u32 {
        self.publish(topic, &Message::text(text))
    }

    /// Publish a binary message to a topic
    ///
    /// @param topic - Topic to publish to
    /// @param data - Binary data to publish
    /// @returns Number of subscribers
    #[napi]
    pub fn publish_binary(&self, topic: String, data: Buffer) -> u32 {
        self.publish(topic, &Message::binary(data))
    }

    /// Get the number of subscribers to a topic
    ///
    /// @param topic - Topic to check
    /// @returns Number of subscribers
    #[napi]
    pub fn subscriber_count(&self, topic: String) -> u32 {
        self.pubsub.topic_subscriber_count(&topic) as u32
    }

    // ==================== State & Stats ====================

    /// Check if the server is running
    #[napi(getter)]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Get server statistics
    #[napi]
    pub fn stats(&self) -> ServerStats {
        ServerStats {
            total_connections: self.stats.total_connections.load(Ordering::Relaxed) as u32,
            active_connections: self.stats.active_connections.load(Ordering::Relaxed) as u32,
            topic_count: self.pubsub.topic_count() as u32,
            messages_published: self.stats.messages_published.load(Ordering::Relaxed) as u32,
        }
    }

    /// Get the port the server is listening on
    #[napi(getter)]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Get the host the server is bound to
    #[napi(getter)]
    pub fn host(&self) -> String {
        self.host.clone()
    }

    /// Get the address string (host:port)
    #[napi(getter)]
    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

async fn run_server(
    listener: TcpListener,
    config: sockudo_ws::Config,
    stats: Arc<ServerStatsInner>,
    running: Arc<AtomicBool>,
    shutdown_tx: broadcast::Sender<()>,
    on_connection: ThreadsafeFunction<(WebSocket, ConnectionInfo)>,
    queue_size: usize,
    pubsub: Arc<sockudo_ws::pubsub::PubSubState>,
) {
    let mut shutdown_rx = shutdown_tx.subscribe();
    let mut connection_id: u32 = 0;
    let on_connection = Arc::new(on_connection);

    loop {
        tokio::select! {
            biased;

            _ = shutdown_rx.recv() => {
                break;
            }

            result = listener.accept() => {
                if !running.load(Ordering::Relaxed) {
                    break;
                }

                match result {
                    Ok((stream, addr)) => {
                        connection_id = connection_id.wrapping_add(1);
                        stats.total_connections.fetch_add(1, Ordering::Relaxed);
                        stats.active_connections.fetch_add(1, Ordering::Relaxed);

                        let config = config.clone();
                        let stats = stats.clone();
                        let on_connection = on_connection.clone();
                        let conn_id = connection_id;
                        let pubsub = pubsub.clone();

                        tokio::spawn(async move {
                            handle_connection(
                                stream,
                                addr,
                                config,
                                stats,
                                on_connection,
                                conn_id,
                                queue_size,
                                pubsub,
                            )
                            .await;
                        });
                    }
                    Err(e) => {
                        eprintln!("Accept error: {}", e);
                    }
                }
            }
        }
    }
}

async fn handle_connection(
    stream: tokio::net::TcpStream,
    addr: SocketAddr,
    config: sockudo_ws::Config,
    stats: Arc<ServerStatsInner>,
    on_connection: Arc<ThreadsafeFunction<(WebSocket, ConnectionInfo)>>,
    connection_id: u32,
    queue_size: usize,
    pubsub: Arc<sockudo_ws::pubsub::PubSubState>,
) {
    let _ = stream.set_nodelay(true);

    let server = sockudo_ws::server::WebSocketServer::<sockudo_ws::Http1>::new(config);

    match server.accept(stream).await {
        Ok((ws_stream, handshake)) => {
            let info = ConnectionInfo {
                remote_addr: addr.to_string(),
                path: handshake.path,
                protocol: handshake.protocol,
                connection_id,
            };

            // Create WebSocket with integrated pub/sub
            let websocket =
                create_websocket_with_pubsub(ws_stream, queue_size, connection_id, pubsub);

            on_connection.call(
                Ok((websocket, info)),
                ThreadsafeFunctionCallMode::NonBlocking,
            );
        }
        Err(e) => {
            stats.active_connections.fetch_sub(1, Ordering::Relaxed);
            eprintln!("Handshake error from {}: {}", addr, e);
        }
    }
}
