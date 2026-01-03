//! Standalone pub/sub system for Node.js
//!
//! This module provides a high-performance pub/sub system that can be used
//! independently of WebSocket connections.

use std::sync::Arc;

use napi::bindgen_prelude::*;
use napi_derive::napi;
use tokio::sync::mpsc;

use crate::message::Message;
use crate::runtime::get_runtime;

/// Result of a publish operation
#[napi(object)]
#[derive(Clone, Debug)]
pub struct PublishResult {
    /// Number of subscribers that received the message
    pub count: u32,
    /// Whether the topic had any subscribers
    pub has_subscribers: bool,
}

/// Pub/Sub statistics
#[napi(object)]
#[derive(Clone, Debug)]
pub struct PubSubStats {
    /// Total number of active topics
    pub topic_count: u32,
    /// Total number of subscribers
    pub subscriber_count: u32,
    /// Total messages published
    pub messages_published: u32,
}

/// A subscriber handle for receiving messages from topics
///
/// Created by calling `pubsub.createSubscriber()`. Use this to subscribe
/// to topics and receive messages via callback.
///
/// @example
/// ```javascript
/// const subscriber = pubsub.createSubscriber();
///
/// subscriber.onMessage((msg) => {
///   console.log('Received:', msg.asText());
/// });
///
/// subscriber.subscribe('chat');
/// subscriber.subscribe('notifications');
///
/// // Later...
/// subscriber.unsubscribe('chat');
/// subscriber.destroy(); // Cleanup when done
/// ```
#[napi]
pub struct Subscriber {
    id: sockudo_ws::pubsub::SubscriberId,
    socket_id: Option<String>,
    pubsub: Arc<sockudo_ws::pubsub::PubSub>,
    #[allow(dead_code)]
    task_handle: Option<tokio::task::JoinHandle<()>>,
    message_callback: Arc<
        parking_lot::RwLock<
            Option<
                napi::threadsafe_function::ThreadsafeFunction<
                    Message,
                    (),
                    Message,
                    napi::Status,
                    false,
                >,
            >,
        >,
    >,
}

#[napi]
impl Subscriber {
    /// Get the subscriber's numeric ID
    #[napi(getter)]
    pub fn id(&self) -> u32 {
        self.id.as_u64() as u32
    }

    /// Get the subscriber's Pusher-style socket ID (if set)
    #[napi(getter)]
    pub fn socket_id(&self) -> Option<String> {
        self.socket_id.clone()
    }

    /// Set the message handler callback
    ///
    /// @param callback - Function called when a message is received
    ///
    /// @example
    /// ```javascript
    /// subscriber.onMessage((msg) => {
    ///   if (msg.isText) {
    ///     console.log('Text:', msg.asText());
    ///   } else {
    ///     console.log('Binary:', msg.asBinary().length, 'bytes');
    ///   }
    /// });
    /// ```
    #[napi]
    pub fn on_message(
        &self,
        #[napi(ts_arg_type = "(message: Message) => void")]
        callback: napi::threadsafe_function::ThreadsafeFunction<
            Message,
            (),
            Message,
            napi::Status,
            false,
        >,
    ) {
        *self.message_callback.write() = Some(callback);
    }

    /// Subscribe to a topic
    ///
    /// @param topic - Topic name to subscribe to
    ///
    /// @example
    /// ```javascript
    /// subscriber.subscribe('chat');
    /// subscriber.subscribe('notifications');
    /// ```
    #[napi]
    pub fn subscribe(&self, topic: String) {
        self.pubsub.subscribe(self.id, &topic);
    }

    /// Unsubscribe from a topic
    ///
    /// @param topic - Topic name to unsubscribe from
    /// @returns true if was subscribed, false otherwise
    #[napi]
    pub fn unsubscribe(&self, topic: String) -> bool {
        self.pubsub.unsubscribe(self.id, &topic)
    }

    /// Check if subscribed to a topic
    ///
    /// @param topic - Topic to check
    /// @returns true if subscribed
    #[napi]
    pub fn is_subscribed(&self, topic: String) -> bool {
        self.pubsub.subscriber_topics(self.id).contains(&topic)
    }

    /// Get all topics this subscriber is subscribed to
    ///
    /// @returns Array of topic names
    #[napi]
    pub fn topics(&self) -> Vec<String> {
        self.pubsub.subscriber_topics(self.id)
    }

    /// Destroy the subscriber and unsubscribe from all topics
    ///
    /// Should be called when you're done with this subscriber to clean up resources.
    #[napi]
    pub fn destroy(&self) {
        self.pubsub.remove_subscriber(self.id);
    }
}

/// High-performance pub/sub system
///
/// A standalone pub/sub system that can be used independently of WebSocket
/// connections. Features:
///
/// - **Sharded topics** (64 shards) for reduced lock contention
/// - **Lock-free subscriber IDs** for fast allocation
/// - **Zero-copy messages** using shared buffers
/// - **Pusher-style socket IDs** for compatibility
///
/// @example
/// ```javascript
/// import { PubSub, Message } from '@sockudo/ws';
///
/// const pubsub = new PubSub();
///
/// // Create subscribers
/// const sub1 = pubsub.createSubscriber();
/// const sub2 = pubsub.createSubscriberWithId('user.123');
///
/// sub1.onMessage((msg) => console.log('Sub1:', msg.asText()));
/// sub2.onMessage((msg) => console.log('Sub2:', msg.asText()));
///
/// sub1.subscribe('chat');
/// sub2.subscribe('chat');
///
/// // Publish to all subscribers
/// pubsub.publish('chat', Message.text('Hello everyone!'));
///
/// // Publish excluding a subscriber
/// pubsub.publishExcluding('chat', Message.text('From sub1'), sub1.id);
///
/// // Cleanup
/// sub1.destroy();
/// sub2.destroy();
/// ```
#[napi]
pub struct PubSub {
    inner: Arc<sockudo_ws::pubsub::PubSub>,
}

#[napi]
impl PubSub {
    /// Create a new pub/sub system
    ///
    /// @example
    /// ```javascript
    /// const pubsub = new PubSub();
    /// ```
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(sockudo_ws::pubsub::PubSub::new()),
        }
    }

    /// Create a new subscriber
    ///
    /// The subscriber will receive messages via the `onMessage` callback.
    ///
    /// @returns A new Subscriber instance
    ///
    /// @example
    /// ```javascript
    /// const subscriber = pubsub.createSubscriber();
    /// subscriber.onMessage((msg) => console.log(msg.asText()));
    /// subscriber.subscribe('chat');
    /// ```
    #[napi]
    pub fn create_subscriber(&self) -> Subscriber {
        let (tx, rx) = mpsc::unbounded_channel();
        let id = self.inner.create_subscriber(tx);
        let message_callback = Arc::new(parking_lot::RwLock::new(None));

        let message_callback_clone = message_callback.clone();
        let task_handle = get_runtime().spawn(async move {
            run_subscriber_loop(rx, message_callback_clone).await;
        });

        Subscriber {
            id,
            socket_id: None,
            pubsub: self.inner.clone(),
            task_handle: Some(task_handle),
            message_callback,
        }
    }

    /// Create a new subscriber with a Pusher-style socket ID
    ///
    /// @param socketId - Custom socket ID (e.g., "user.123" or "1234.5678")
    /// @returns A new Subscriber instance
    ///
    /// @example
    /// ```javascript
    /// const subscriber = pubsub.createSubscriberWithId('user.123');
    /// console.log(subscriber.socketId); // 'user.123'
    /// ```
    #[napi]
    pub fn create_subscriber_with_id(&self, socket_id: String) -> Subscriber {
        let (tx, rx) = mpsc::unbounded_channel();
        let id = self.inner.create_subscriber_with_id(&socket_id, tx);
        let message_callback = Arc::new(parking_lot::RwLock::new(None));

        let message_callback_clone = message_callback.clone();
        let task_handle = get_runtime().spawn(async move {
            run_subscriber_loop(rx, message_callback_clone).await;
        });

        Subscriber {
            id,
            socket_id: Some(socket_id),
            pubsub: self.inner.clone(),
            task_handle: Some(task_handle),
            message_callback,
        }
    }

    /// Generate a Pusher-style socket ID
    ///
    /// Format: `{random}.{random}` (e.g., "1234567890.9876543210")
    ///
    /// @returns A new unique socket ID
    ///
    /// @example
    /// ```javascript
    /// const socketId = PubSub.generateSocketId();
    /// const subscriber = pubsub.createSubscriberWithId(socketId);
    /// ```
    #[napi]
    pub fn generate_socket_id() -> String {
        sockudo_ws::pubsub::PubSub::generate_socket_id()
    }

    /// Publish a message to all subscribers of a topic
    ///
    /// @param topic - Topic to publish to
    /// @param message - Message to publish
    /// @returns Result with subscriber count
    ///
    /// @example
    /// ```javascript
    /// const result = pubsub.publish('chat', Message.text('Hello!'));
    /// console.log(`Sent to ${result.count} subscribers`);
    /// ```
    #[napi]
    pub fn publish(&self, topic: String, message: &Message) -> PublishResult {
        let msg: sockudo_ws::Message = message.into();
        let result = self.inner.publish(&topic, msg);

        match result {
            sockudo_ws::pubsub::PublishResult::Published(count) => PublishResult {
                count: count as u32,
                has_subscribers: true,
            },
            sockudo_ws::pubsub::PublishResult::NoSubscribers => PublishResult {
                count: 0,
                has_subscribers: false,
            },
        }
    }

    /// Publish a text message to a topic
    ///
    /// @param topic - Topic to publish to
    /// @param text - Text to publish
    /// @returns Result with subscriber count
    #[napi]
    pub fn publish_text(&self, topic: String, text: String) -> PublishResult {
        self.publish(topic, &Message::text(text))
    }

    /// Publish a binary message to a topic
    ///
    /// @param topic - Topic to publish to
    /// @param data - Binary data to publish
    /// @returns Result with subscriber count
    #[napi]
    pub fn publish_binary(&self, topic: String, data: Buffer) -> PublishResult {
        self.publish(topic, &Message::binary(data))
    }

    /// Publish a message to all subscribers except one
    ///
    /// @param topic - Topic to publish to
    /// @param message - Message to publish
    /// @param excludeId - Subscriber ID to exclude
    /// @returns Result with subscriber count
    ///
    /// @example
    /// ```javascript
    /// // Broadcast to everyone except the sender
    /// pubsub.publishExcluding('chat', Message.text('Hello!'), sender.id);
    /// ```
    #[napi]
    pub fn publish_excluding(
        &self,
        topic: String,
        message: &Message,
        exclude_id: u32,
    ) -> PublishResult {
        let msg: sockudo_ws::Message = message.into();
        let exclude = sockudo_ws::pubsub::SubscriberId::from_u64(exclude_id as u64);
        let result = self.inner.publish_excluding(exclude, &topic, msg);

        match result {
            sockudo_ws::pubsub::PublishResult::Published(count) => PublishResult {
                count: count as u32,
                has_subscribers: true,
            },
            sockudo_ws::pubsub::PublishResult::NoSubscribers => PublishResult {
                count: 0,
                has_subscribers: false,
            },
        }
    }

    /// Publish excluding a subscriber by socket ID
    ///
    /// @param topic - Topic to publish to
    /// @param message - Message to publish
    /// @param excludeSocketId - Socket ID to exclude
    /// @returns Result with subscriber count
    #[napi]
    pub fn publish_excluding_socket_id(
        &self,
        topic: String,
        message: &Message,
        exclude_socket_id: String,
    ) -> PublishResult {
        let msg: sockudo_ws::Message = message.into();
        let result = self
            .inner
            .publish_excluding_socket_id(&exclude_socket_id, &topic, msg);

        match result {
            sockudo_ws::pubsub::PublishResult::Published(count) => PublishResult {
                count: count as u32,
                has_subscribers: true,
            },
            sockudo_ws::pubsub::PublishResult::NoSubscribers => PublishResult {
                count: 0,
                has_subscribers: false,
            },
        }
    }

    /// Get the number of subscribers to a topic
    ///
    /// @param topic - Topic to check
    /// @returns Number of subscribers
    #[napi]
    pub fn subscriber_count(&self, topic: String) -> u32 {
        self.inner.topic_subscriber_count(&topic) as u32
    }

    /// Get the total number of topics with at least one subscriber
    ///
    /// @returns Number of active topics
    #[napi(getter)]
    pub fn topic_count(&self) -> u32 {
        self.inner.topic_count() as u32
    }

    /// Get the total number of subscribers
    ///
    /// @returns Number of subscribers
    #[napi(getter)]
    pub fn total_subscribers(&self) -> u32 {
        self.inner.subscriber_count() as u32
    }

    /// Get the total number of messages published
    ///
    /// @returns Number of messages
    #[napi(getter)]
    pub fn messages_published(&self) -> u32 {
        self.inner.messages_published() as u32
    }

    /// Get pub/sub statistics
    ///
    /// @returns Statistics object
    #[napi]
    pub fn stats(&self) -> PubSubStats {
        PubSubStats {
            topic_count: self.inner.topic_count() as u32,
            subscriber_count: self.inner.subscriber_count() as u32,
            messages_published: self.inner.messages_published() as u32,
        }
    }

    /// Get a subscriber by their socket ID
    ///
    /// @param socketId - Socket ID to look up
    /// @returns Subscriber ID if found, undefined otherwise
    #[napi]
    pub fn get_subscriber_by_socket_id(&self, socket_id: String) -> Option<u32> {
        self.inner
            .get_subscriber_by_socket_id(&socket_id)
            .map(|id| id.as_u64() as u32)
    }

    /// Get all socket IDs in the system
    ///
    /// @returns Array of socket IDs
    #[napi]
    pub fn all_socket_ids(&self) -> Vec<String> {
        self.inner.all_socket_ids()
    }
}

async fn run_subscriber_loop(
    mut rx: mpsc::UnboundedReceiver<sockudo_ws::Message>,
    message_callback: Arc<
        parking_lot::RwLock<
            Option<
                napi::threadsafe_function::ThreadsafeFunction<
                    Message,
                    (),
                    Message,
                    napi::Status,
                    false,
                >,
            >,
        >,
    >,
) {
    use napi::threadsafe_function::ThreadsafeFunctionCallMode;

    while let Some(msg) = rx.recv().await {
        let napi_msg: Message = msg.into();
        if let Some(cb) = message_callback.read().as_ref() {
            cb.call(napi_msg, ThreadsafeFunctionCallMode::NonBlocking);
        }
    }
}
