const { WebSocketClient, Message, Compression } = require('../index.js');

async function main() {
  console.log('Connecting to WebSocket server...');

  const client = new WebSocketClient({
    url: 'ws://localhost:8080',
    config: {
      compression: Compression.Disabled,
      idleTimeout: 60
    }
  });

  await client.connect();
  console.log('Connected!');

  // NAPI-RS callbacks use error-first pattern: (err, data)
  client.onMessage((err, msg) => {
    if (msg.isText) {
      console.log(`Server says: ${msg.asText()}`);
    } else if (msg.isBinary) {
      console.log(`Received binary: ${msg.asBuffer().length} bytes`);
    }
  });

  client.onClose((err, reason) => {
    console.log(`Connection closed: ${reason.code} - ${reason.reason}`);
  });

  client.onError((err, error) => {
    console.error(`Error: ${error}`);
  });

  // Send some messages
  client.sendText('Hello from Node.js!');

  setTimeout(() => {
    client.sendText('This is message 2');
  }, 1000);

  setTimeout(() => {
    client.sendBinary(Buffer.from([0x01, 0x02, 0x03, 0x04]));
  }, 2000);

  setTimeout(() => {
    console.log('\nConnection stats:', client.stats());
    client.close(1000, 'Goodbye!');
  }, 3000);

  setTimeout(() => {
    process.exit(0);
  }, 4000);
}

main().catch(console.error);
