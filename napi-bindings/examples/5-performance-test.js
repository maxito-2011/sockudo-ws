const { WebSocketServer, WebSocketClient, Message, Compression } = require('../index.js');

const PORT = 8082;
const NUM_CLIENTS = 100;
const MESSAGES_PER_CLIENT = 100;

async function runPerformanceTest() {
  console.log('Starting performance test...\n');

  const server = new WebSocketServer({
    port: PORT,
    config: {
      compression: Compression.Disabled,
      maxMessageSize: 1024 * 1024,
      idleTimeout: 300
    }
  });

  let serverMessagesReceived = 0;
  let serverMessagesSent = 0;

  // NAPI-RS callbacks use error-first pattern: (err, data)
  server.start((err, data) => {
    const [ws, info] = data;
    ws.onMessage((err, msg) => {
      serverMessagesReceived++;
      ws.send(msg);
      serverMessagesSent++;
    });

    ws.onError((err, error) => {
      console.error('Server error:', error);
    });
  });

  console.log(`Server started on port ${PORT}`);

  await new Promise(resolve => setTimeout(resolve, 500));

  console.log(`Spawning ${NUM_CLIENTS} clients...`);

  const startTime = Date.now();
  const clients = [];

  for (let i = 0; i < NUM_CLIENTS; i++) {
    const client = new WebSocketClient({
      url: `ws://localhost:${PORT}`,
      config: {
        compression: Compression.Disabled
      }
    });

    let messagesReceived = 0;
    let messagesSent = 0;

    client.onMessage((_, msg) => {
      messagesReceived++;
    });

    client.onError((_, error) => {
      console.error(`Client ${i} error:`, error);
    });

    await client.connect();
    clients.push({ client, messagesReceived, messagesSent, id: i });
  }

  console.log(`All ${NUM_CLIENTS} clients connected`);
  console.log(`Sending ${MESSAGES_PER_CLIENT} messages per client...\n`);

  const sendStartTime = Date.now();

  for (const clientData of clients) {
    for (let j = 0; j < MESSAGES_PER_CLIENT; j++) {
      clientData.client.sendText(`Message ${j} from client ${clientData.id}`);
      clientData.messagesSent++;
    }
  }

  console.log('All messages sent, waiting for responses...');

  await new Promise(resolve => setTimeout(resolve, 2000));

  const endTime = Date.now();
  const totalTime = endTime - startTime;
  const sendTime = endTime - sendStartTime;

  const totalMessagesSent = clients.reduce((sum, c) => sum + c.messagesSent, 0);
  const totalMessagesReceived = clients.reduce((sum, c) => sum + c.messagesReceived, 0);

  console.log('\n=== Performance Test Results ===');
  console.log(`Total time: ${totalTime}ms`);
  console.log(`Send time: ${sendTime}ms`);
  console.log(`Clients: ${NUM_CLIENTS}`);
  console.log(`Messages per client: ${MESSAGES_PER_CLIENT}`);
  console.log(`Total messages sent: ${totalMessagesSent}`);
  console.log(`Total messages received: ${totalMessagesReceived}`);
  console.log(`Server messages received: ${serverMessagesReceived}`);
  console.log(`Server messages sent: ${serverMessagesSent}`);
  console.log(`Messages per second: ${Math.round((totalMessagesSent + totalMessagesReceived) / (sendTime / 1000))}`);
  console.log(`Round-trip latency: ${(sendTime / totalMessagesSent).toFixed(2)}ms`);

  console.log('\n=== Server Stats ===');
  console.log(JSON.stringify(server.stats(), null, 2));

  console.log('\nCleaning up...');

  for (const clientData of clients) {
    clientData.client.close();
  }

  await new Promise(resolve => setTimeout(resolve, 500));

  server.stop();

  console.log('Performance test complete!');
  process.exit(0);
}

runPerformanceTest().catch(console.error);
