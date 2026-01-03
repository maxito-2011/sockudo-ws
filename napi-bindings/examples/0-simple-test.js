const { WebSocketServer, WebSocketClient, Message } = require('../index.js');

async function test() {
  console.log('Creating server...');

  const server = new WebSocketServer({
    port: 8090,
    host: '0.0.0.0'
  });

  let connectionReceived = false;

  // NOTE: NAPI-RS uses error-first callback convention
  // Callbacks receive (err, data) where err is always null for successful operations
  server.start((err, data) => {
    const [ws, info] = data;
    console.log(`Server: Client connected from ${info.remoteAddr}`);
    connectionReceived = true;

    ws.onMessage((err, msg) => {
      console.log(`Server: Received message: ${msg.asText()}`);
      ws.sendText(`Echo: ${msg.asText()}`);
    });

    ws.onClose((err, reason) => {
      console.log(`Server: Client disconnected: ${reason.code}`);
    });

    ws.onError((err, error) => {
      console.error(`Server: Error: ${error}`);
    });

    console.log('Server: Sending welcome message');
    ws.sendText('Welcome!');
  });

  console.log('Server started, waiting 1 second...');
  await new Promise(resolve => setTimeout(resolve, 1000));

  console.log('Creating client...');

  const client = new WebSocketClient({
    url: 'ws://localhost:8090'
  });

  console.log('Client: Connecting...');
  try {
    await client.connect();
    console.log('Client: Connected!');
  } catch (error) {
    console.error('Client: Failed to connect:', error);
    console.log('Server running:', server.isRunning);
    console.log('Server port:', server.port);
    throw error;
  }

  client.onMessage((err, msg) => {
    console.log(`Client: Received: ${msg.asText()}`);
  });

  client.onClose((err, reason) => {
    console.log(`Client: Connection closed: ${reason.code} - ${reason.reason}`);
  });

  client.onError((err, error) => {
    console.error(`Client: Error: ${error}`);
  });

  console.log('Client: Sending message...');
  client.sendText('Hello from client!');

  await new Promise(resolve => setTimeout(resolve, 2000));

  console.log('Client: Closing connection...');
  client.close(1000, 'Test complete');

  await new Promise(resolve => setTimeout(resolve, 500));

  console.log('Stopping server...');
  server.stop();

  console.log('Test complete!');
  process.exit(0);
}

test().catch((error) => {
  console.error('Test failed:', error);
  process.exit(1);
});
