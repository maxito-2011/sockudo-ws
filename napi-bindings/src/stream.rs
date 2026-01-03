//! WebSocket stream with lock-free message handling and integrated pub/sub

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crossbeam_channel::{Receiver, Sender, TrySendError, bounded, unbounded};
use futures_util::{SinkExt, StreamExt};
use napi::bindgen_prelude::*;
use napi::threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode};
use napi_derive::napi;
use parking_lot::RwLock;

use crate::error::CloseReason;
use crate::message::Message;
use crate::runtime::get_runtime;

/// Statistics for a WebSocket connection
#[napi(object)]
#[derive(Clone, Debug, Default)]
pub struct ConnectionStats {
    /// Total messages sent
    pub messages_sent: u32,
    /// Total messages received
    pub messages_received: u32,
    /// Total bytes sent
    pub bytes_sent: u32,
    /// Total bytes received
    pub bytes_received: u32,
    /// Current write buffer size
    pub write_buffer_size: u32,
    /// Whether the connection is backpressured
    pub is_backpressured: bool,
}

pub struct ConnectionStatsInner {
    pub messages_sent: AtomicU64,
    pub messages_received: AtomicU64,
    pub bytes_sent: AtomicU64,
    pub bytes_received: AtomicU64,
    pub write_buffer_size: AtomicU64,
    pub is_backpressured: AtomicBool,
}

impl Default for ConnectionStatsInner {
    fn default() -> Self {
        Self {
            messages_sent: AtomicU64::new(0),
            messages_received: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            write_buffer_size: AtomicU64::new(0),
            is_backpressured: AtomicBool::new(false),
        }
    }
}

pub enum SendCommand {
    Message(sockudo_ws::Message),
    Subscribe(String),
    Unsubscribe(String),
    Publish(String, sockudo_ws::Message),
    Close(u16, String),
}

/// High-performance WebSocket connection with lock-free queues and pub/sub support
///
/// This class wraps a WebSocket connection and provides:
/// - Lock-free message sending via crossbeam channels
/// - Event-based message receiving with callbacks
/// - Pub/sub topic-based messaging (subscribe, unsubscribe, publish)
/// - Backpressure monitoring
/// - Connection statistics
///
/// @example
/// ```javascript
/// // Inside connection handler
/// server.start((ws, info) => {
///   // Subscribe to topics
///   ws.subscribe('chat');
///   ws.subscribe('notifications');
///
///   ws.onMessage((msg) => {
///     if (msg.isText) {
///       // Broadcast to all subscribers except sender
///       ws.publish('chat', msg);
///     }
///   });
///
///   ws.onClose((reason) => {
///     console.log(`Closed: ${reason.code}`);
///   });
/// });
/// ```
#[napi]
pub struct WebSocket {
    /// Unique connection ID
    connection_id: u32,
    /// Subscriber ID for pub/sub (used internally for cleanup)
    #[allow(dead_code)]
    subscriber_id: Option<sockudo_ws::pubsub::SubscriberId>,
    /// Subscribed topics (local cache)
    topics: Arc<RwLock<HashSet<String>>>,
    /// Lock-free send channel
    send_tx: Sender<SendCommand>,
    /// Connection state
    closed: Arc<AtomicBool>,
    /// Statistics
    stats: Arc<ConnectionStatsInner>,
    /// Message callback
    message_callback:
        Arc<RwLock<Option<ThreadsafeFunction<Message, (), Message, napi::Status, false>>>>,
    /// Close callback
    close_callback:
        Arc<RwLock<Option<ThreadsafeFunction<CloseReason, (), CloseReason, napi::Status, false>>>>,
    /// Error callback
    error_callback:
        Arc<RwLock<Option<ThreadsafeFunction<String, (), String, napi::Status, false>>>>,
}

#[napi]
impl WebSocket {
    // ==================== Event Handlers ====================

    /// Set the message handler callback
    ///
    /// @param callback - Function called when a message is received
    ///
    /// @example
    /// ```javascript
    /// ws.onMessage((msg) => {
    ///   if (msg.isText) {
    ///     console.log(msg.asText());
    ///   }
    /// });
    /// ```
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
    ///
    /// @example
    /// ```javascript
    /// ws.onClose((reason) => {
    ///   console.log(`Closed: ${reason.code}`);
    /// });
    /// ```
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
    ///
    /// @example
    /// ```javascript
    /// ws.onError((error) => {
    ///   console.error('WebSocket error:', error);
    /// });
    /// ```
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

    /// Send a message through the WebSocket connection
    ///
    /// Uses a lock-free channel for maximum performance.
    /// The message is queued and sent asynchronously.
    ///
    /// @param message - The message to send
    /// @returns true if queued successfully, false if connection is closed or queue is full
    ///
    /// @example
    /// ```javascript
    /// ws.send(Message.text("Hello!"));
    /// ws.send(Message.binary(Buffer.from([1, 2, 3])));
    /// ```
    #[napi]
    pub fn send(&self, message: &Message) -> bool {
        if self.closed.load(Ordering::Relaxed) {
            return false;
        }

        let msg: sockudo_ws::Message = message.into();
        let size = match &msg {
            sockudo_ws::Message::Text(b) | sockudo_ws::Message::Binary(b) => b.len(),
            sockudo_ws::Message::Ping(b) | sockudo_ws::Message::Pong(b) => b.len(),
            sockudo_ws::Message::Close(_) => 0,
        };

        match self.send_tx.try_send(SendCommand::Message(msg)) {
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

    /// Close the WebSocket connection
    ///
    /// @param code - Close code (default: 1000)
    /// @param reason - Close reason (default: empty)
    #[napi]
    pub fn close(&self, code: Option<u32>, reason: Option<String>) {
        if self.closed.swap(true, Ordering::SeqCst) {
            return; // Already closed
        }

        let close_code = code.unwrap_or(1000) as u16;
        let close_reason = reason.unwrap_or_default();

        let _ = self
            .send_tx
            .try_send(SendCommand::Close(close_code, close_reason));
    }

    // ==================== Pub/Sub ====================

    /// Subscribe to a topic
    ///
    /// Messages published to this topic will be received by this socket
    /// (except messages published by this socket itself via `publish()`).
    ///
    /// @param topic - Topic name to subscribe to
    ///
    /// @example
    /// ```javascript
    /// ws.subscribe('chat');
    /// ws.subscribe('notifications');
    /// ws.subscribe(`user/${userId}`);
    /// ```
    #[napi]
    pub fn subscribe(&self, topic: String) {
        if self.closed.load(Ordering::Relaxed) {
            return;
        }
        self.topics.write().insert(topic.clone());
        let _ = self.send_tx.try_send(SendCommand::Subscribe(topic));
    }

    /// Unsubscribe from a topic
    ///
    /// @param topic - Topic name to unsubscribe from
    ///
    /// @example
    /// ```javascript
    /// ws.unsubscribe('chat');
    /// ```
    #[napi]
    pub fn unsubscribe(&self, topic: String) {
        if self.closed.load(Ordering::Relaxed) {
            return;
        }
        self.topics.write().remove(&topic);
        let _ = self.send_tx.try_send(SendCommand::Unsubscribe(topic));
    }

    /// Check if subscribed to a topic
    ///
    /// @param topic - Topic name to check
    /// @returns true if subscribed
    ///
    /// @example
    /// ```javascript
    /// if (ws.isSubscribed('chat')) {
    ///   console.log('Subscribed to chat');
    /// }
    /// ```
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

    /// Publish a message to all subscribers of a topic (except this socket)
    ///
    /// The message is sent to ALL subscribers of the topic EXCEPT this socket.
    /// This is the same behavior as Bun's `ws.publish()`.
    ///
    /// @param topic - Topic to publish to
    /// @param message - Message to publish
    /// @returns true if queued successfully
    ///
    /// @example
    /// ```javascript
    /// // Broadcast chat message to everyone except sender
    /// ws.publish('chat', Message.text(`${username}: ${text}`));
    /// ```
    #[napi]
    pub fn publish(&self, topic: String, message: &Message) -> bool {
        if self.closed.load(Ordering::Relaxed) {
            return false;
        }
        let msg: sockudo_ws::Message = message.into();
        self.send_tx
            .try_send(SendCommand::Publish(topic, msg))
            .is_ok()
    }

    /// Publish a text message to a topic
    ///
    /// @param topic - Topic to publish to
    /// @param text - Text to publish
    /// @returns true if queued successfully
    #[napi]
    pub fn publish_text(&self, topic: String, text: String) -> bool {
        self.publish(topic, &Message::text(text))
    }

    /// Publish a binary message to a topic
    ///
    /// @param topic - Topic to publish to
    /// @param data - Binary data to publish
    /// @returns true if queued successfully
    #[napi]
    pub fn publish_binary(&self, topic: String, data: Buffer) -> bool {
        self.publish(topic, &Message::binary(data))
    }

    // ==================== State & Stats ====================

    /// Get the connection ID (unique per server)
    #[napi(getter)]
    pub fn id(&self) -> u32 {
        self.connection_id
    }

    /// Check if the connection is closed
    #[napi(getter)]
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Relaxed)
    }

    /// Check if the write buffer is backpressured
    ///
    /// When true, you should pause sending until the buffer drains.
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
}

/// Internal: Create a new WebSocket wrapper from a stream with pub/sub support
pub(crate) fn create_websocket_with_pubsub<S>(
    stream: sockudo_ws::WebSocketStream<S>,
    queue_size: usize,
    connection_id: u32,
    pubsub: Arc<sockudo_ws::pubsub::PubSubState>,
) -> WebSocket
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let (send_tx, send_rx) = if queue_size > 0 {
        bounded(queue_size)
    } else {
        unbounded()
    };

    let closed = Arc::new(AtomicBool::new(false));
    let stats = Arc::new(ConnectionStatsInner::default());
    let topics = Arc::new(RwLock::new(HashSet::new()));

    let message_callback = Arc::new(RwLock::new(None));
    let close_callback = Arc::new(RwLock::new(None));
    let error_callback = Arc::new(RwLock::new(None));

    // Create subscriber in pub/sub system
    let (msg_tx, msg_rx) = tokio::sync::mpsc::unbounded_channel();
    let subscriber_id = pubsub.create_subscriber(msg_tx);

    let closed_clone = closed.clone();
    let stats_clone = stats.clone();
    let message_cb = message_callback.clone();
    let close_cb = close_callback.clone();
    let error_cb = error_callback.clone();
    let pubsub_clone = pubsub.clone();

    // Spawn the WebSocket handler task
    get_runtime().spawn(async move {
        // Small delay to allow callbacks to be set
        tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;

        run_websocket_loop_with_pubsub(
            stream,
            send_rx,
            msg_rx,
            closed_clone,
            stats_clone,
            message_cb,
            close_cb,
            error_cb,
            pubsub_clone,
            subscriber_id,
        )
        .await;
    });

    WebSocket {
        connection_id,
        subscriber_id: Some(subscriber_id),
        topics,
        send_tx,
        closed,
        stats,
        message_callback,
        close_callback,
        error_callback,
    }
}

async fn run_websocket_loop_with_pubsub<S>(
    mut stream: sockudo_ws::WebSocketStream<S>,
    send_rx: Receiver<SendCommand>,
    mut pubsub_rx: tokio::sync::mpsc::UnboundedReceiver<sockudo_ws::Message>,
    closed: Arc<AtomicBool>,
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
    pubsub: Arc<sockudo_ws::pubsub::PubSubState>,
    subscriber_id: sockudo_ws::pubsub::SubscriberId,
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

            // Handle incoming WebSocket messages
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
                            break;
                        }
                    }
                    None => {
                        closed.store(true, Ordering::SeqCst);
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

            // Handle pub/sub messages from other connections
            Some(msg) = pubsub_rx.recv() => {
                if let Err(e) = stream.send(msg).await {
                    if let Some(cb) = error_callback.read().as_ref() {
                        cb.call(e.to_string(), ThreadsafeFunctionCallMode::NonBlocking);
                    }
                    if e.is_fatal() {
                        closed.store(true, Ordering::SeqCst);
                        break;
                    }
                }
            }

            // Handle outgoing commands
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
                                    break;
                                }
                            }
                        }
                        SendCommand::Subscribe(topic) => {
                            pubsub.subscribe(subscriber_id, &topic);
                        }
                        SendCommand::Unsubscribe(topic) => {
                            pubsub.unsubscribe(subscriber_id, &topic);
                        }
                        SendCommand::Publish(topic, msg) => {
                            pubsub.publish_excluding(subscriber_id, &topic, msg);
                        }
                        SendCommand::Close(code, reason) => {
                            let close_msg = sockudo_ws::Message::Close(Some(
                                sockudo_ws::error::CloseReason::new(code, &reason)
                            ));
                            let _ = stream.send(close_msg).await;
                            let _ = stream.flush().await;
                            closed.store(true, Ordering::SeqCst);
                            break;
                        }
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

    // Cleanup: remove subscriber from pub/sub
    pubsub.remove_subscriber(subscriber_id);

    // Final flush
    let _ = stream.flush().await;
}
