const { PubSubServer, Message, Compression } = require('../index.js');

console.log('Starting pub/sub chat server...');

const server = new PubSubServer({
  port: 8081,
  host: '0.0.0.0',
  config: {
    compression: Compression.Shared,
    idleTimeout: 120,
    autoPing: true
  }
});

// NAPI-RS callbacks use error-first pattern: (err, data)
server.start((err, data) => {
  const [ws, info] = data;
  console.log(`User ${info.connectionId} connected from ${info.remoteAddr}`);

  // Subscribe to the general chat room
  ws.subscribe('chat/general');
  ws.subscribe(`user/${info.connectionId}`);

  // Send welcome message
  ws.sendText(JSON.stringify({
    type: 'welcome',
    userId: info.connectionId,
    message: 'Welcome to the chat room!'
  }));

  // Announce new user to everyone else
  ws.publishText('chat/general', JSON.stringify({
    type: 'system',
    message: `User ${info.connectionId} joined the chat`
  }));

  ws.onMessage((err, msg) => {
    if (msg.isText) {
      try {
        const data = JSON.parse(msg.asText());

        switch (data.type) {
          case 'chat':
            // Broadcast to all subscribers except sender
            ws.publishText('chat/general', JSON.stringify({
              type: 'message',
              userId: info.connectionId,
              message: data.message,
              timestamp: Date.now()
            }));
            break;

          case 'private':
            // Send private message to specific user
            ws.publishText(`user/${data.targetUserId}`, JSON.stringify({
              type: 'private',
              fromUserId: info.connectionId,
              message: data.message,
              timestamp: Date.now()
            }));
            break;

          case 'stats':
            // Send stats to this user only
            const stats = ws.stats();
            const serverStats = server.stats();
            ws.sendText(JSON.stringify({
              type: 'stats',
              connection: stats,
              server: serverStats,
              subscriberCount: server.subscriberCount('chat/general')
            }));
            break;
        }
      } catch (e) {
        console.error('Failed to parse message:', e.message);
      }
    }
  });

  ws.onClose((err, reason) => {
    console.log(`User ${info.connectionId} disconnected: ${reason.code}`);

    // Announce user left
    server.publishText('chat/general', JSON.stringify({
      type: 'system',
      message: `User ${info.connectionId} left the chat`
    }));
  });

  ws.onError((err, error) => {
    console.error(`WebSocket error for user ${info.connectionId}:`, error);
  });
});

console.log(`Pub/Sub chat server listening on port ${server.port}`);
console.log('Subscriptions:');
console.log('  - chat/general: main chat room');
console.log('  - user/{id}: private messages');
console.log('\nPress Ctrl+C to stop');

// Log stats every 10 seconds
const statsInterval = setInterval(() => {
  const stats = server.stats();
  console.log(`\nStats: ${stats.activeConnections} active, ${stats.totalConnections} total, ${stats.topicCount} topics, ${stats.messagesPublished} published`);
}, 10000);

process.on('SIGINT', () => {
  console.log('\nStopping server...');
  clearInterval(statsInterval);
  server.stop();
  process.exit(0);
});
