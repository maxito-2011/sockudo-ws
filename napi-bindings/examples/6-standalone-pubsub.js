/**
 * Standalone Pub/Sub Example
 *
 * This example demonstrates using the high-performance pub/sub system
 * independently of WebSocket connections. Perfect for:
 *
 * - Inter-process communication
 * - Event bus patterns
 * - Message routing
 * - Real-time data distribution
 *
 * Features demonstrated:
 * - Creating subscribers (with auto-generated and custom socket IDs)
 * - Topic subscription and unsubscription
 * - Publishing to all subscribers
 * - Publishing with exclusions (broadcast to others)
 * - Statistics and monitoring
 */

const { PubSub, Message } = require('../index.js');

console.log('=== Standalone Pub/Sub Example ===\n');

// Create a new pub/sub system
const pubsub = new PubSub();

// Track messages received by each subscriber for demonstration
const receivedMessages = {
  alice: [],
  bob: [],
  charlie: []
};

// ============================================================================
// Create subscribers
// ============================================================================

console.log('Creating subscribers...\n');

// Subscriber with auto-generated ID
const alice = pubsub.createSubscriber();
console.log(`Alice created with ID: ${alice.id}`);

// Subscribers with custom Pusher-style socket IDs
const bobSocketId = 'user.bob.12345';
const bob = pubsub.createSubscriberWithId(bobSocketId);
console.log(`Bob created with socket ID: ${bob.socketId}`);

// Using the static method to generate a socket ID
const charlieSocketId = PubSub.generateSocketId();
const charlie = pubsub.createSubscriberWithId(charlieSocketId);
console.log(`Charlie created with generated socket ID: ${charlie.socketId}`);

// ============================================================================
// Set up message handlers
// ============================================================================

console.log('\nSetting up message handlers...\n');

alice.onMessage((msg) => {
  const text = msg.isText ? msg.asText() : `[binary: ${msg.asBinary().length} bytes]`;
  receivedMessages.alice.push(text);
  console.log(`  [Alice] Received: ${text}`);
});

bob.onMessage((msg) => {
  const text = msg.isText ? msg.asText() : `[binary: ${msg.asBinary().length} bytes]`;
  receivedMessages.bob.push(text);
  console.log(`  [Bob] Received: ${text}`);
});

charlie.onMessage((msg) => {
  const text = msg.isText ? msg.asText() : `[binary: ${msg.asBinary().length} bytes]`;
  receivedMessages.charlie.push(text);
  console.log(`  [Charlie] Received: ${text}`);
});

// ============================================================================
// Subscribe to topics
// ============================================================================

console.log('Subscribing to topics...\n');

// All subscribe to 'announcements'
alice.subscribe('announcements');
bob.subscribe('announcements');
charlie.subscribe('announcements');
console.log(`'announcements' topic has ${pubsub.subscriberCount('announcements')} subscribers`);

// Alice and Bob subscribe to 'private-chat'
alice.subscribe('private-chat');
bob.subscribe('private-chat');
console.log(`'private-chat' topic has ${pubsub.subscriberCount('private-chat')} subscribers`);

// Only Charlie subscribes to 'admin'
charlie.subscribe('admin');
console.log(`'admin' topic has ${pubsub.subscriberCount('admin')} subscribers`);

// ============================================================================
// Demonstrate publishing
// ============================================================================

console.log('\n--- Publishing Messages ---\n');

// 1. Publish to all subscribers of a topic
console.log('1. Publishing to "announcements" (all 3 subscribers):');
let result = pubsub.publishText('announcements', 'Hello everyone!');
console.log(`   Result: sent to ${result.count} subscribers\n`);

// Small delay to let async callbacks fire
await sleep(10);

// 2. Publish excluding a specific subscriber by ID
console.log('2. Publishing to "announcements" excluding Alice:');
result = pubsub.publishExcluding(
  'announcements',
  Message.text('This message excludes Alice'),
  alice.id
);
console.log(`   Result: sent to ${result.count} subscribers\n`);

await sleep(10);

// 3. Publish excluding by socket ID
console.log('3. Publishing to "private-chat" excluding Bob (by socket ID):');
result = pubsub.publishExcludingSocketId(
  'private-chat',
  Message.text('Secret message Bob cannot see'),
  bobSocketId
);
console.log(`   Result: sent to ${result.count} subscribers\n`);

await sleep(10);

// 4. Publish to a topic with only one subscriber
console.log('4. Publishing to "admin" (only Charlie):');
result = pubsub.publishText('admin', 'Admin-only notification');
console.log(`   Result: sent to ${result.count} subscribers\n`);

await sleep(10);

// 5. Publish binary data
console.log('5. Publishing binary data to "announcements":');
const binaryData = Buffer.from([0x01, 0x02, 0x03, 0x04, 0x05]);
result = pubsub.publishBinary('announcements', binaryData);
console.log(`   Result: sent to ${result.count} subscribers\n`);

await sleep(10);

// 6. Publish to non-existent topic
console.log('6. Publishing to non-existent topic:');
result = pubsub.publishText('non-existent', 'Nobody will receive this');
console.log(`   Result: sent to ${result.count} subscribers, hasSubscribers: ${result.hasSubscribers}\n`);

// ============================================================================
// Query operations
// ============================================================================

console.log('--- Query Operations ---\n');

// Check subscriptions
console.log(`Alice's topics: ${JSON.stringify(alice.topics())}`);
console.log(`Bob's topics: ${JSON.stringify(bob.topics())}`);
console.log(`Charlie's topics: ${JSON.stringify(charlie.topics())}`);

// Check if subscribed
console.log(`\nIs Alice subscribed to 'admin'? ${alice.isSubscribed('admin')}`);
console.log(`Is Charlie subscribed to 'admin'? ${charlie.isSubscribed('admin')}`);

// Lookup by socket ID
console.log(`\nLookup Bob by socket ID: ${pubsub.getSubscriberBySocketId(bobSocketId)}`);
console.log(`All socket IDs: ${JSON.stringify(pubsub.allSocketIds())}`);

// ============================================================================
// Statistics
// ============================================================================

console.log('\n--- Statistics ---\n');

const stats = pubsub.stats();
console.log(`Topic count: ${stats.topicCount}`);
console.log(`Subscriber count: ${stats.subscriberCount}`);
console.log(`Messages published: ${stats.messagesPublished}`);

// Alternative getters
console.log(`\nUsing getters:`);
console.log(`  pubsub.topicCount: ${pubsub.topicCount}`);
console.log(`  pubsub.totalSubscribers: ${pubsub.totalSubscribers}`);
console.log(`  pubsub.messagesPublished: ${pubsub.messagesPublished}`);

// ============================================================================
// Unsubscribe and cleanup
// ============================================================================

console.log('\n--- Unsubscribe and Cleanup ---\n');

// Unsubscribe from a topic
console.log(`Unsubscribing Alice from 'announcements'...`);
const wasSubscribed = alice.unsubscribe('announcements');
console.log(`Was subscribed: ${wasSubscribed}`);
console.log(`'announcements' now has ${pubsub.subscriberCount('announcements')} subscribers`);

// Publish after unsubscribe
console.log('\nPublishing to "announcements" after Alice unsubscribed:');
result = pubsub.publishText('announcements', 'Alice will not see this');
console.log(`Result: sent to ${result.count} subscribers`);

await sleep(10);

// Destroy subscribers (cleanup)
console.log('\nDestroying all subscribers...');
alice.destroy();
bob.destroy();
charlie.destroy();

console.log(`\nAfter cleanup:`);
console.log(`  Topic count: ${pubsub.topicCount}`);
console.log(`  Subscriber count: ${pubsub.totalSubscribers}`);

// ============================================================================
// Summary
// ============================================================================

console.log('\n--- Message Summary ---\n');
console.log(`Alice received ${receivedMessages.alice.length} messages`);
console.log(`Bob received ${receivedMessages.bob.length} messages`);
console.log(`Charlie received ${receivedMessages.charlie.length} messages`);

console.log('\n=== Example Complete ===\n');

// Helper function
function sleep(ms) {
  return new Promise(resolve => setTimeout(resolve, ms));
}
