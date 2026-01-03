const { WebSocketServer, Message, Compression } = require('../index.js');

console.log('Starting echo server...');

const server = new WebSocketServer({
  port: 8080,
  host: '0.0.0.0',
  config: {
    compression: Compression.Disabled,
    maxMessageSize: 16 * 1024 * 1024,
    idleTimeout: 60,
    autoPing: true,
    pingInterval: 30
  }
});

// NAPI-RS callbacks use error-first pattern: (err, data)
server.start((err, data) => {
  const [ws, info] = data;
  console.log(`Client connected from ${info.remoteAddr} (ID: ${info.connectionId})`);

  ws.onMessage((err, msg) => {
    if (msg.isText) {
      const text = msg.asText();
      console.log(`Received text: ${text}`);
      ws.send(Message.text(`Echo: ${text}`));
    } else if (msg.isBinary) {
      const buffer = msg.asBuffer();
      console.log(`Received binary data: ${buffer.length} bytes`);
      ws.send(msg);
    }
  });

  ws.onClose((err, reason) => {
    console.log(`Client disconnected: ${reason.code} - ${reason.reason}`);
  });

  ws.onError((err, error) => {
    console.error(`WebSocket error: ${error}`);
  });

  ws.sendText('Welcome to the echo server!');
});

console.log(`Echo server listening on port ${server.port}`);
console.log('Press Ctrl+C to stop');

process.on('SIGINT', () => {
  console.log('\nStopping server...');
  server.stop();
  process.exit(0);
});
