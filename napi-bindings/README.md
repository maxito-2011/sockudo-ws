# @sockudo/ws

<div align="center">

**Ultra-fast WebSocket library for Node.js**

[![npm version](https://img.shields.io/npm/v/@sockudo/ws.svg)](https://www.npmjs.com/package/@sockudo/ws)
[![JSR](https://jsr.io/badges/@sockudo/ws)](https://jsr.io/@sockudo/ws)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

*17% faster than alternatives • Matches uWebSockets performance • Zero-copy • SIMD-accelerated*

</div>

---

## Features

- **Blazing Fast** - Built on Rust's sockudo-ws, 17% faster than other Rust WebSocket libraries
- **Lock-Free** - Uses crossbeam lock-free queues for zero-contention message passing
- **SIMD Accelerated** - AVX2/AVX-512/NEON for frame masking and UTF-8 validation
- **Zero-Copy** - Shared buffers minimize memory allocations
- **Pub/Sub System** - High-performance topic-based messaging with Bun-compatible API
- **Configurable Runtime** - Control Tokio worker threads for your workload
- **Backpressure Support** - Built-in flow control for high-throughput scenarios
- **Compression** - permessage-deflate (RFC 7692) with multiple modes
- **Statistics** - Real-time connection and message statistics

## Installation

```bash
# npm
npm install @sockudo/ws

# pnpm
pnpm add @sockudo/ws

# yarn
yarn add @sockudo/ws

# bun
bun add @sockudo/ws
```

## Quick Start

### Server

```typescript
import { WebSocketServer, Message, Compression } from '@sockudo/ws';

const server = new WebSocketServer({
  port: 8080,
  host: '0.0.0.0',
  config: {
    compression: Compression.Shared,
    maxMessageSize: 16 * 1024 * 1024, // 16MB
  }
});

await server.start((ws, info) => {
  console.log(`New connection from ${info.remoteAddr} on ${info.path}`);

  ws.onMessage((msg) => {
    if (msg.isText) {
      console.log('Received:', msg.asText());
    }
    // Echo back
    ws.send(msg);
  });

  ws.onClose((reason) => {
    console.log(`Connection closed: ${reason.code} - ${reason.reason}`);
  });

  ws.onError((error) => {
    console.error('WebSocket error:', error);
  });
});

console.log(`Server listening on ws://localhost:${server.port}`);
```

### Client

```typescript
import { WebSocketClient, Message } from '@sockudo/ws';

const client = new WebSocketClient({
  url: 'ws://localhost:8080/ws'
});

await client.connect();

client.onMessage((msg) => {
  console.log('Received:', msg.asText());
});

client.onClose((reason) => {
  console.log('Disconnected:', reason.code);
});

// Send messages
client.sendText('Hello, World!');
client.send(Message.binary(Buffer.from([1, 2, 3, 4])));

// Close when done
client.close(1000, 'Goodbye');
```

### Quick Connect

```typescript
import { connect } from '@sockudo/ws';

const client = await connect('ws://localhost:8080/ws');
client.onMessage((msg) => console.log(msg.asText()));
client.sendText('Hello!');
```

## API Reference

### Runtime Configuration

```typescript
import { initRuntime, getWorkerThreads, getAvailableCores } from '@sockudo/ws';

// Initialize with 4 worker threads
initRuntime(4);

// Or use all available cores (default)
initRuntime(0);

console.log(`Using ${getWorkerThreads()} threads out of ${getAvailableCores()} cores`);
```

### Message Types

```typescript
import { Message, MessageType } from '@sockudo/ws';

// Create messages
const text = Message.text('Hello');
const binary = Message.binary(Buffer.from([1, 2, 3]));
const ping = Message.ping();
const pong = Message.pong(Buffer.from('payload'));
const close = Message.close(1000, 'Normal closure');

// Check message type
if (msg.isText) {
  console.log(msg.asText());
} else if (msg.isBinary) {
  console.log(msg.asBuffer());
} else if (msg.isClose) {
  const reason = msg.closeReason();
  console.log(`Close: ${reason?.code}`);
}

// Message properties
console.log(msg.messageType); // MessageType.Text, etc.
console.log(msg.size);        // Payload size in bytes
console.log(msg.isControl);   // true for ping/pong/close
```

### Configuration

```typescript
import { Config, Compression, hftConfig, throughputConfig, uwsConfig } from '@sockudo/ws';

// Custom config
const config: Config = {
  maxMessageSize: 64 * 1024 * 1024,  // 64MB
  maxFrameSize: 16 * 1024 * 1024,    // 16MB
  writeBufferSize: 16 * 1024,        // 16KB
  compression: Compression.Shared,
  idleTimeout: 120,                   // seconds
  maxBackpressure: 1024 * 1024,      // 1MB
  autoPing: true,
  pingInterval: 30,                   // seconds
  highWaterMark: 64 * 1024,          // 64KB
  lowWaterMark: 16 * 1024,           // 16KB
};

// Preset configs
const hft = hftConfig();        // Optimized for low latency (HFT)
const throughput = throughputConfig(); // Optimized for high throughput
const uws = uwsConfig();        // uWebSockets-compatible defaults
```

### Compression Modes

```typescript
import { Compression } from '@sockudo/ws';

Compression.Disabled   // No compression
Compression.Dedicated  // Per-connection compressor (best ratio, more memory)
Compression.Shared     // Shared compressor (balanced)
Compression.Shared4KB  // Shared with 4KB window
Compression.Shared8KB  // Shared with 8KB window
Compression.Shared16KB // Shared with 16KB window
```

### WebSocket Connection

```typescript
// Available on both server connections and client
ws.send(message);           // Send a Message object
ws.sendText('hello');       // Send text
ws.sendBinary(buffer);      // Send binary
ws.ping();                  // Send ping
ws.close(1000, 'reason');   // Close connection

// Properties
ws.isClosed;                // Check if closed
ws.isBackpressured;         // Check backpressure

// Statistics
const stats = ws.stats();
console.log(stats.messagesSent);
console.log(stats.messagesReceived);
console.log(stats.bytesSent);
console.log(stats.bytesReceived);
console.log(stats.writeBufferSize);
console.log(stats.isBackpressured);
```

### Server Options

```typescript
interface ServerOptions {
  port: number;           // Required: port to listen on
  host?: string;          // Default: "0.0.0.0"
  config?: Config;        // WebSocket configuration
  queueSize?: number;     // Send queue size per connection (default: 1024, 0 = unbounded)
}
```

### Client Options

```typescript
interface ClientOptions {
  url: string;            // Required: ws:// or wss:// URL
  config?: Config;        // WebSocket configuration
  protocol?: string;      // WebSocket subprotocol
  queueSize?: number;     // Send queue size (default: 1024)
}
```

### Connection Info (Server)

```typescript
interface ConnectionInfo {
  remoteAddr: string;     // Client IP:port
  path: string;           // Request path
  protocol: string | null; // Negotiated subprotocol
  connectionId: number;   // Unique ID per server
}
```

### Close Codes

```typescript
import { CloseCode } from '@sockudo/ws';

CloseCode.Normal          // 1000
CloseCode.GoingAway       // 1001
CloseCode.ProtocolError   // 1002
CloseCode.Unsupported     // 1003
CloseCode.NoStatus        // 1005
CloseCode.Abnormal        // 1006
CloseCode.InvalidPayload  // 1007
CloseCode.PolicyViolation // 1008
CloseCode.MessageTooBig   // 1009
CloseCode.MissingExtension// 1010
CloseCode.InternalError   // 1011
```

## Pub/Sub API

The library includes a high-performance Pub/Sub system with a Bun-compatible API. This is ideal for building chat applications, real-time notifications, and broadcasting systems.

### Standalone PubSub Class

For custom use cases, you can use the standalone `PubSub` class independently of WebSocket:

```typescript
import { PubSub, Message } from '@sockudo/ws';

const pubsub = new PubSub();

// Create subscribers
const sub1 = pubsub.createSubscriber();
const sub2 = pubsub.createSubscriberWithId('user.123'); // Pusher-style ID

// Set up message handlers
sub1.onMessage((msg) => console.log('Sub1:', msg.asText()));
sub2.onMessage((msg) => console.log('Sub2:', msg.asText()));

// Subscribe to topics
sub1.subscribe('chat');
sub2.subscribe('chat');

// Publish to all subscribers
const result = pubsub.publish('chat', Message.text('Hello everyone!'));
console.log(`Sent to ${result.count} subscribers`);

// Publish excluding a subscriber (like Bun's ws.publish)
pubsub.publishExcluding('chat', Message.text('From sub1'), sub1.id);

// Using socket IDs
pubsub.publishExcludingSocketId('chat', Message.text('Hello'), 'user.123');

// Stats
console.log(pubsub.stats()); // { topicCount, subscriberCount, messagesPublished }

// Cleanup
sub1.destroy();
sub2.destroy();
```

### Subscriber API

```typescript
// Properties
subscriber.id;                    // Numeric ID
subscriber.socketId;              // Pusher-style socket ID (if set)

// Methods
subscriber.subscribe('topic');    // Subscribe to a topic
subscriber.unsubscribe('topic');  // Unsubscribe from a topic
subscriber.isSubscribed('topic'); // Check subscription status
subscriber.topics();              // Get all subscribed topics
subscriber.onMessage(callback);   // Set message handler
subscriber.destroy();             // Cleanup (unsubscribe from all topics)
```

### PubSub API

```typescript
// Create subscribers
pubsub.createSubscriber();                      // Auto-generated ID
pubsub.createSubscriberWithId('socket.id');     // Custom Pusher-style ID
PubSub.generateSocketId();                      // Generate Pusher-style ID

// Publishing
pubsub.publish('topic', message);               // To all subscribers
pubsub.publishText('topic', 'text');            // Convenience method
pubsub.publishBinary('topic', buffer);          // Convenience method
pubsub.publishExcluding('topic', msg, id);      // Exclude by numeric ID
pubsub.publishExcludingSocketId('topic', msg, 'socket.id'); // Exclude by socket ID

// Stats & queries
pubsub.subscriberCount('topic');                // Subscribers in topic
pubsub.topicCount;                              // Total active topics
pubsub.totalSubscribers;                        // Total subscribers
pubsub.messagesPublished;                       // Total messages sent
pubsub.stats();                                 // All stats
pubsub.getSubscriberBySocketId('socket.id');    // Lookup by socket ID
pubsub.allSocketIds();                          // All registered socket IDs
```

### Pub/Sub Server

```typescript
import { PubSubServer, Message, Compression } from '@sockudo/ws';

const server = new PubSubServer({
  port: 8080,
  host: '0.0.0.0',
  config: {
    compression: Compression.Shared,
    maxMessageSize: 16 * 1024 * 1024,
  }
});

await server.start((ws, info) => {
  console.log(`New connection from ${info.remoteAddr}`);
  
  // Subscribe to topics
  ws.subscribe('chat/general');
  ws.subscribe(`user/${info.connectionId}`);
  
  ws.onMessage((msg) => {
    if (msg.isText) {
      // Broadcast to all subscribers EXCEPT the sender
      ws.publish('chat/general', msg);
    }
  });
  
  ws.onClose((reason) => {
    console.log(`Connection closed: ${reason.code}`);
  });
});

// Server-level publish (reaches ALL subscribers)
server.publish('chat/general', Message.text('Server announcement!'));

console.log(`Pub/Sub server listening on port ${server.port}`);
```

### Topic Subscriptions

```typescript
// Subscribe to multiple topics
ws.subscribe('chat/general');
ws.subscribe('chat/private');
ws.subscribe('notifications');

// Unsubscribe from a topic
ws.unsubscribe('chat/private');

// Check subscription status
if (ws.isSubscribed('chat/general')) {
  console.log('Subscribed to general chat');
}

// Get all subscriptions
console.log('Topics:', ws.subscriptions);
```

### Publishing Messages

```typescript
// ws.publish() - Sends to all subscribers EXCEPT the sender
// This is the same behavior as Bun's ServerWebSocket.publish()
ws.publish('chat/general', Message.text('Hello from client!'));
ws.publishText('chat/general', 'Hello!');
ws.publishBinary('data', Buffer.from([1, 2, 3]));

// server.publish() - Sends to ALL subscribers (including sender)
// Use this for server announcements
server.publish('announcements', Message.text('Server restarting in 5 minutes'));
server.publishText('announcements', 'Maintenance scheduled');
server.publishBinary('updates', Buffer.from([0x01, 0x02]));
```

### Direct Messages

```typescript
// Send directly to a specific socket (not through topics)
ws.send(Message.text('Private message'));
ws.sendText('Hello!');
ws.sendBinary(Buffer.from([1, 2, 3]));
```

### Pub/Sub Statistics

```typescript
// Server statistics
const stats = server.stats();
console.log(`Total connections: ${stats.totalConnections}`);
console.log(`Active connections: ${stats.activeConnections}`);
console.log(`Active topics: ${stats.topicCount}`);
console.log(`Messages published: ${stats.messagesPublished}`);

// Get subscriber count for a topic
const count = server.subscriberCount('chat/general');
console.log(`Subscribers in general chat: ${count}`);

// Connection statistics
const connStats = ws.stats();
console.log(`Messages sent: ${connStats.messagesSent}`);
console.log(`Messages received: ${connStats.messagesReceived}`);
```

### Pub/Sub Server Options

```typescript
interface PubSubServerOptions {
  port: number;           // Required: port to listen on
  host?: string;          // Default: "0.0.0.0"
  config?: Config;        // WebSocket configuration
  queueSize?: number;     // Send queue size per connection (default: 1024)
}
```

### Chat Room Example

```typescript
import { PubSubServer, Message } from '@sockudo/ws';

const server = new PubSubServer({ port: 8080 });

const users = new Map<number, string>();

await server.start((ws, info) => {
  const userId = info.connectionId;
  
  ws.onMessage((msg) => {
    if (!msg.isText) return;
    
    const data = JSON.parse(msg.asText()!);
    
    switch (data.type) {
      case 'join':
        users.set(userId, data.username);
        ws.subscribe(`room/${data.room}`);
        ws.publishText(`room/${data.room}`, JSON.stringify({
          type: 'user_joined',
          username: data.username
        }));
        break;
        
      case 'message':
        ws.publishText(`room/${data.room}`, JSON.stringify({
          type: 'message',
          username: users.get(userId),
          text: data.text
        }));
        break;
        
      case 'leave':
        ws.unsubscribe(`room/${data.room}`);
        ws.publishText(`room/${data.room}`, JSON.stringify({
          type: 'user_left',
          username: users.get(userId)
        }));
        break;
    }
  });
  
  ws.onClose(() => {
    users.delete(userId);
  });
});
```

## Performance Tips

### 1. Configure Worker Threads

```typescript
import { initRuntime, getAvailableCores } from '@sockudo/ws';

// For CPU-bound workloads, use all cores
initRuntime(0);

// For I/O-bound with many connections, use 2x cores
initRuntime(getAvailableCores() * 2);

// For mixed workloads, tune based on profiling
initRuntime(4);
```

### 2. Handle Backpressure

```typescript
ws.onMessage((msg) => {
  // Check before sending to avoid queue buildup
  if (!ws.isBackpressured) {
    ws.send(msg);
  } else {
    // Buffer, drop, or wait
    console.log('Backpressured, buffering...');
  }
});
```

### 3. Use Binary Messages for Large Data

```typescript
// More efficient than text for large payloads
const data = Buffer.from(JSON.stringify(largeObject));
ws.send(Message.binary(data));
```

### 4. Tune Queue Size

```typescript
// For high-throughput, increase queue size
const server = new WebSocketServer({
  port: 8080,
  queueSize: 4096, // Larger queue for bursty traffic
});

// For low-latency, use smaller queue
const hftServer = new WebSocketServer({
  port: 8080,
  queueSize: 64, // Smaller queue = faster feedback
  config: hftConfig(),
});
```

### 5. Enable Compression for Text-Heavy Workloads

```typescript
const server = new WebSocketServer({
  port: 8080,
  config: {
    compression: Compression.Shared, // Good balance
    // compression: Compression.Dedicated, // Better ratio, more memory
  }
});
```

## Benchmarks

Tested on AMD Ryzen 9 5900X, 64GB RAM, Ubuntu 22.04:

| Library | Messages/sec | Latency p99 |
|---------|-------------|-------------|
| @sockudo/ws | **1,250,000** | **45μs** |
| uWebSockets | 1,200,000 | 48μs |
| ws | 380,000 | 180μs |
| websocket | 320,000 | 210μs |

*Echo server benchmark with 1KB messages, 100 concurrent connections*

## Platform Support

| Platform | Architecture | Status |
|----------|-------------|--------|
| Windows | x64 | ✅ |
| Windows | ARM64 | ✅ |
| macOS | x64 | ✅ |
| macOS | ARM64 (Apple Silicon) | ✅ |
| Linux (glibc) | x64 | ✅ |
| Linux (glibc) | ARM64 | ✅ |
| Linux (glibc) | ARMv7 | ✅ |
| Linux (musl) | x64 | ✅ |
| Linux (musl) | ARM64 | ✅ |
| FreeBSD | x64 | ✅ |

## Comparison with ws

```typescript
// ws (JavaScript)
import WebSocket from 'ws';
const wss = new WebSocket.Server({ port: 8080 });
wss.on('connection', (ws) => {
  ws.on('message', (data) => ws.send(data));
});

// @sockudo/ws (Rust-powered)
import { WebSocketServer } from '@sockudo/ws';
const server = new WebSocketServer({ port: 8080 });
await server.start((ws, info) => {
  ws.onMessage((msg) => ws.send(msg));
});
```

Key differences:
- **3x+ faster** message throughput
- **4x lower** latency
- **Lock-free** architecture
- **Native** binary (no pure JS overhead)
- **SIMD** acceleration
- **Built-in** statistics

## License

MIT License - see [LICENSE](LICENSE) for details.

## Contributing

Contributions are welcome! Please read our [Contributing Guide](CONTRIBUTING.md) for details.

## Links

- [GitHub Repository](https://github.com/sockudo/sockudo-ws)
- [npm Package](https://www.npmjs.com/package/@sockudo/ws)
- [JSR Package](https://jsr.io/@sockudo/ws)
- [Rust Crate (sockudo-ws)](https://crates.io/crates/sockudo-ws)
