//! WebSocket client implementation

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crossbeam_channel::{Sender, TrySendError, bounded, unbounded};
use futures_util::{SinkExt, StreamExt};
use napi::bindgen_prelude::*;
use napi::threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode};
use napi_derive::napi;
use parking_lot::RwLock;

use crate::config::Config;
use crate::error::CloseReason;
use crate::message::Message;
use crate::runtime::get_runtime;
use crate::stream::{ConnectionStats, ConnectionStatsInner, SendCommand};

/// Options for connecting to a WebSocket server
#[napi(object)]
#[derive(Clone, Debug)]
pub struct ClientOptions {
    /// WebSocket URL (ws:// or wss://)
    pub url: String,
    /// WebSocket configuration
    pub config: Option<Config>,
    /// WebSocket subprotocol to request
    pub protocol: Option<String>,
    /// Send queue size (0 = unbounded, default: 1024)
    pub queue_size: Option<u32>,
}

/// High-performance WebSocket client
///
/// Connects to a WebSocket server using HTTP/1.1 upgrade.
/// Shares the same API as the server-side WebSocket for consistency.
///
/// @example
/// ```javascript
/// import { WebSocketClient, Message } from '@sockudo/ws';
///
/// const client = new WebSocketClient({
///   url: 'ws://localhost:8080/ws'
/// });
///
/// await client.connect();
///
/// client.onMessage((msg) => {
///   console.log('Received:', msg.asText());
/// });
///
/// client.onClose((reason) => {
///   console.log('Closed:', reason.code);
/// });
///
/// client.sendText('Hello!');
/// ```
#[napi]
pub struct WebSocketClient {
    url: String,
    config: sockudo_ws::Config,
    protocol: Option<String>,
    queue_size: usize,
    // Connection state
    send_tx: Arc<RwLock<Option<Sender<SendCommand>>>>,
    closed: Arc<AtomicBool>,
    connected: Arc<AtomicBool>,
    stats: Arc<ConnectionStatsInner>,
    topics: Arc<RwLock<HashSet<String>>>,
    // Callbacks
    message_callback:
        Arc<RwLock<Option<ThreadsafeFunction<Message, (), Message, napi::Status, false>>>>,
    close_callback:
        Arc<RwLock<Option<ThreadsafeFunction<CloseReason, (), CloseReason, napi::Status, false>>>>,
    error_callback:
        Arc<RwLock<Option<ThreadsafeFunction<String, (), String, napi::Status, false>>>>,
}

#[napi]
impl WebSocketClient {
    /// Create a new WebSocket client
    ///
    /// @param options - Client options including URL and config
    ///
    /// @example
    /// ```javascript
    /// const client = new WebSocketClient({
    ///   url: 'ws://localhost:8080/ws',
    ///   config: { compression: Compression.Shared }
    /// });
    /// ```
    #[napi(constructor)]
    pub fn new(options: ClientOptions) -> Self {
        let config: sockudo_ws::Config = options.config.unwrap_or_default().into();

        Self {
            url: options.url,
            config,
            protocol: options.protocol,
            queue_size: options.queue_size.unwrap_or(1024) as usize,
            send_tx: Arc::new(RwLock::new(None)),
            closed: Arc::new(AtomicBool::new(false)),
            connected: Arc::new(AtomicBool::new(false)),
            stats: Arc::new(ConnectionStatsInner::default()),
            topics: Arc::new(RwLock::new(HashSet::new())),
            message_callback: Arc::new(RwLock::new(None)),
            close_callback: Arc::new(RwLock::new(None)),
            error_callback: Arc::new(RwLock::new(None)),
        }
    }

    /// Connect to the WebSocket server
    ///
    /// @returns Promise that resolves when connected
    ///
    /// @example
    /// ```javascript
    /// await client.connect();
    /// console.log('Connected!');
    /// ```
    #[napi]
    pub async fn connect(&self) -> Result<()> {
        if self.connected.load(Ordering::Relaxed) {
            return Err(Error::new(Status::GenericFailure, "Already connected"));
        }

        let url = self.url.clone();
        let config = self.config.clone();
        let protocol = self.protocol.clone();
        let queue_size = self.queue_size;

        let send_tx_holder = self.send_tx.clone();
        let closed = self.closed.clone();
        let connected = self.connected.clone();
        let stats = self.stats.clone();
        let message_callback = self.message_callback.clone();
        let close_callback = self.close_callback.clone();
        let error_callback = self.error_callback.clone();

        get_runtime()
            .spawn(async move {
                let ws_client =
                    sockudo_ws::client::WebSocketClient::<sockudo_ws::Http1>::new(config);

                match ws_client.connect_to_url(&url, protocol.as_deref()).await {
                    Ok((stream, _handshake)) => {
                        let (send_tx, send_rx) = if queue_size > 0 {
                            bounded(queue_size)
                        } else {
                            unbounded()
                        };

                        *send_tx_holder.write() = Some(send_tx);
                        connected.store(true, Ordering::SeqCst);

                        // Spawn the client loop
                        run_client_loop(
                            stream,
                            send_rx,
                            closed,
                            connected,
                            stats,
                            message_callback,
                            close_callback,
                            error_callback,
                        )
                        .await;

                        Ok(())
                    }
                    Err(e) => Err(Error::new(
                        Status::GenericFailure,
                        format!("Connection failed: {}", e),
                    )),
                }
            })
            .await
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
    }

    // ==================== Event Handlers ====================

    /// Set the message handler callback
    ///
    /// @param callback - Function called when a message is received
    #[napi]
    pub fn on_message(
        &self,
        #[napi(ts_arg_type = "(message: Message) => void")] callback: ThreadsafeFunction<
            Message,
            (),
            Message,
            napi::Status,
            false,
        >,
    ) {
        *self.message_callback.write() = Some(callback);
    }

    /// Set the close handler callback
    ///
    /// @param callback - Function called when the connection closes
    #[napi]
    pub fn on_close(
        &self,
        #[napi(ts_arg_type = "(reason: CloseReason) => void")] callback: ThreadsafeFunction<
            CloseReason,
            (),
            CloseReason,
            napi::Status,
            false,
        >,
    ) {
        *self.close_callback.write() = Some(callback);
    }

    /// Set the error handler callback
    ///
    /// @param callback - Function called when an error occurs
    #[napi]
    pub fn on_error(
        &self,
        #[napi(ts_arg_type = "(error: string) => void")] callback: ThreadsafeFunction<
            String,
            (),
            String,
            napi::Status,
            false,
        >,
    ) {
        *self.error_callback.write() = Some(callback);
    }

    // ==================== Sending ====================

    /// Send a message
    ///
    /// @param message - The message to send
    /// @returns true if queued successfully
    #[napi]
    pub fn send(&self, message: &Message) -> bool {
        if self.closed.load(Ordering::Relaxed) || !self.connected.load(Ordering::Relaxed) {
            return false;
        }

        let msg: sockudo_ws::Message = message.into();
        let size = match &msg {
            sockudo_ws::Message::Text(b) | sockudo_ws::Message::Binary(b) => b.len(),
            sockudo_ws::Message::Ping(b) | sockudo_ws::Message::Pong(b) => b.len(),
            sockudo_ws::Message::Close(_) => 0,
        };

        if let Some(tx) = self.send_tx.read().as_ref() {
            match tx.try_send(SendCommand::Message(msg)) {
                Ok(()) => {
                    self.stats.messages_sent.fetch_add(1, Ordering::Relaxed);
                    self.stats
                        .bytes_sent
                        .fetch_add(size as u64, Ordering::Relaxed);
                    true
                }
                Err(TrySendError::Full(_)) => {
                    self.stats.is_backpressured.store(true, Ordering::Relaxed);
                    false
                }
                Err(TrySendError::Disconnected(_)) => {
                    self.closed.store(true, Ordering::Relaxed);
                    false
                }
            }
        } else {
            false
        }
    }

    /// Send a text message
    ///
    /// @param text - The text to send
    /// @returns true if queued successfully
    #[napi]
    pub fn send_text(&self, text: String) -> bool {
        self.send(&Message::text(text))
    }

    /// Send a binary message
    ///
    /// @param data - The binary data to send
    /// @returns true if queued successfully
    #[napi]
    pub fn send_binary(&self, data: Buffer) -> bool {
        self.send(&Message::binary(data))
    }

    /// Send a ping message
    ///
    /// @param data - Optional ping payload
    /// @returns true if queued successfully
    #[napi]
    pub fn ping(&self, data: Option<Buffer>) -> bool {
        self.send(&Message::ping(data))
    }

    /// Close the connection
    ///
    /// @param code - Close code (default: 1000)
    /// @param reason - Close reason
    #[napi]
    pub fn close(&self, code: Option<u32>, reason: Option<String>) {
        if self.closed.swap(true, Ordering::SeqCst) {
            return;
        }
        self.connected.store(false, Ordering::SeqCst);

        if let Some(tx) = self.send_tx.read().as_ref() {
            let _ = tx.try_send(SendCommand::Close(
                code.unwrap_or(1000) as u16,
                reason.unwrap_or_default(),
            ));
        }
    }

    // ==================== Pub/Sub (for symmetry with server WebSocket) ====================

    /// Subscribe to a topic (client-side tracking only)
    ///
    /// Note: For client connections, this tracks subscriptions locally.
    /// The server must handle the actual pub/sub logic.
    ///
    /// @param topic - Topic name to subscribe to
    #[napi]
    pub fn subscribe(&self, topic: String) {
        self.topics.write().insert(topic);
    }

    /// Unsubscribe from a topic
    ///
    /// @param topic - Topic name to unsubscribe from
    #[napi]
    pub fn unsubscribe(&self, topic: String) {
        self.topics.write().remove(&topic);
    }

    /// Check if subscribed to a topic
    ///
    /// @param topic - Topic name to check
    /// @returns true if subscribed
    #[napi]
    pub fn is_subscribed(&self, topic: String) -> bool {
        self.topics.read().contains(&topic)
    }

    /// Get all subscribed topics
    ///
    /// @returns Array of topic names
    #[napi(getter)]
    pub fn subscriptions(&self) -> Vec<String> {
        self.topics.read().iter().cloned().collect()
    }

    // ==================== State & Stats ====================

    /// Check if connected
    #[napi(getter)]
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed) && !self.closed.load(Ordering::Relaxed)
    }

    /// Check if the connection is closed
    #[napi(getter)]
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Relaxed)
    }

    /// Check if backpressured
    #[napi(getter)]
    pub fn is_backpressured(&self) -> bool {
        self.stats.is_backpressured.load(Ordering::Relaxed)
    }

    /// Get connection statistics
    #[napi]
    pub fn stats(&self) -> ConnectionStats {
        ConnectionStats {
            messages_sent: self.stats.messages_sent.load(Ordering::Relaxed) as u32,
            messages_received: self.stats.messages_received.load(Ordering::Relaxed) as u32,
            bytes_sent: self.stats.bytes_sent.load(Ordering::Relaxed) as u32,
            bytes_received: self.stats.bytes_received.load(Ordering::Relaxed) as u32,
            write_buffer_size: self.stats.write_buffer_size.load(Ordering::Relaxed) as u32,
            is_backpressured: self.stats.is_backpressured.load(Ordering::Relaxed),
        }
    }

    /// Get the URL
    #[napi(getter)]
    pub fn url(&self) -> String {
        self.url.clone()
    }
}

/// Connect to a WebSocket server (convenience function)
///
/// @param url - WebSocket URL
/// @param config - Optional configuration
/// @returns Connected WebSocket client
///
/// @example
/// ```javascript
/// import { connect, Message } from '@sockudo/ws';
///
/// const client = await connect('ws://localhost:8080/ws');
/// client.onMessage((msg) => console.log(msg.asText()));
/// client.sendText('Hello!');
/// ```
#[napi]
pub async fn connect(url: String, config: Option<Config>) -> Result<WebSocketClient> {
    let client = WebSocketClient::new(ClientOptions {
        url,
        config,
        protocol: None,
        queue_size: Some(1024),
    });

    client.connect().await?;
    Ok(client)
}

async fn run_client_loop<S>(
    mut stream: sockudo_ws::WebSocketStream<S>,
    send_rx: crossbeam_channel::Receiver<SendCommand>,
    closed: Arc<AtomicBool>,
    connected: Arc<AtomicBool>,
    stats: Arc<ConnectionStatsInner>,
    message_callback: Arc<
        RwLock<Option<ThreadsafeFunction<Message, (), Message, napi::Status, false>>>,
    >,
    close_callback: Arc<
        RwLock<Option<ThreadsafeFunction<CloseReason, (), CloseReason, napi::Status, false>>>,
    >,
    error_callback: Arc<
        RwLock<Option<ThreadsafeFunction<String, (), String, napi::Status, false>>>,
    >,
) where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    use tokio::time::{Duration, interval};

    let mut send_interval = interval(Duration::from_micros(50));

    loop {
        if closed.load(Ordering::Relaxed) {
            break;
        }

        tokio::select! {
            biased;

            msg = stream.next() => {
                match msg {
                    Some(Ok(msg)) => {
                        let size = msg.as_bytes().len() as u64;
                        stats.messages_received.fetch_add(1, Ordering::Relaxed);
                        stats.bytes_received.fetch_add(size, Ordering::Relaxed);

                        if msg.is_close() {
                            let close_reason = if let sockudo_ws::Message::Close(reason) = &msg {
                                reason.clone().map(|r| CloseReason {
                                    code: r.code,
                                    reason: r.reason,
                                }).unwrap_or(CloseReason {
                                    code: 1000,
                                    reason: String::new(),
                                })
                            } else {
                                CloseReason { code: 1000, reason: String::new() }
                            };

                            if let Some(cb) = close_callback.read().as_ref() {
                                cb.call(close_reason, ThreadsafeFunctionCallMode::NonBlocking);
                            }
                            closed.store(true, Ordering::SeqCst);
                            connected.store(false, Ordering::SeqCst);
                            break;
                        }

                        let napi_msg: Message = msg.into();
                        if let Some(cb) = message_callback.read().as_ref() {
                            cb.call(napi_msg, ThreadsafeFunctionCallMode::NonBlocking);
                        }
                    }
                    Some(Err(e)) => {
                        if let Some(cb) = error_callback.read().as_ref() {
                            cb.call(e.to_string(), ThreadsafeFunctionCallMode::NonBlocking);
                        }
                        if e.is_fatal() {
                            closed.store(true, Ordering::SeqCst);
                            connected.store(false, Ordering::SeqCst);
                            break;
                        }
                    }
                    None => {
                        closed.store(true, Ordering::SeqCst);
                        connected.store(false, Ordering::SeqCst);
                        if let Some(cb) = close_callback.read().as_ref() {
                            cb.call(
                                CloseReason { code: 1006, reason: "Connection closed".to_string() },
                                ThreadsafeFunctionCallMode::NonBlocking
                            );
                        }
                        break;
                    }
                }
            }

            _ = send_interval.tick() => {
                let mut batch_count = 0;
                while let Ok(cmd) = send_rx.try_recv() {
                    batch_count += 1;
                    match cmd {
                        SendCommand::Message(msg) => {
                            if let Err(e) = stream.send(msg).await {
                                if let Some(cb) = error_callback.read().as_ref() {
                                    cb.call(e.to_string(), ThreadsafeFunctionCallMode::NonBlocking);
                                }
                                if e.is_fatal() {
                                    closed.store(true, Ordering::SeqCst);
                                    connected.store(false, Ordering::SeqCst);
                                    return;
                                }
                            }
                        }
                        SendCommand::Close(code, reason) => {
                            let close_msg = sockudo_ws::Message::Close(Some(
                                sockudo_ws::error::CloseReason::new(code, &reason)
                            ));
                            let _ = stream.send(close_msg).await;
                            let _ = stream.flush().await;
                            closed.store(true, Ordering::SeqCst);
                            connected.store(false, Ordering::SeqCst);
                            return;
                        }
                        // Client doesn't handle pub/sub commands
                        _ => {}
                    }

                    if batch_count >= 100 {
                        break;
                    }
                }

                if batch_count > 0 {
                    let _ = stream.flush().await;
                }

                let is_bp = stream.is_backpressured();
                stats.is_backpressured.store(is_bp, Ordering::Relaxed);
                stats.write_buffer_size.store(stream.write_buffer_len() as u64, Ordering::Relaxed);
            }
        }
    }

    let _ = stream.flush().await;
}
