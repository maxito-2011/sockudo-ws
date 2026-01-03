const { WebSocketClient, Message } = require('../index.js');
const readline = require('readline');

async function main() {
  console.log('Connecting to chat server...');

  const client = new WebSocketClient({
    url: 'ws://localhost:8081'
  });

  let myUserId = null;

  // NAPI-RS callbacks use error-first pattern: (err, data)
  client.onMessage((err, msg) => {
    if (msg.isText) {
      try {
        const data = JSON.parse(msg.asText());

        switch (data.type) {
          case 'welcome':
            myUserId = data.userId;
            console.log(`\n${data.message}`);
            console.log(`Your user ID is: ${myUserId}`);
            console.log('\nCommands:');
            console.log('  Type a message to send to everyone');
            console.log('  /private <userId> <message> - Send private message');
            console.log('  /stats - Show statistics');
            console.log('  /quit - Exit\n');
            break;

          case 'message':
            console.log(`[User ${data.userId}]: ${data.message}`);
            break;

          case 'private':
            console.log(`[Private from User ${data.fromUserId}]: ${data.message}`);
            break;

          case 'system':
            console.log(`[System] ${data.message}`);
            break;

          case 'stats':
            console.log('\n=== Statistics ===');
            console.log('Connection:', JSON.stringify(data.connection, null, 2));
            console.log('Server:', JSON.stringify(data.server, null, 2));
            console.log('Subscribers in chat/general:', data.subscriberCount);
            console.log('==================\n');
            break;
        }
      } catch (e) {
        console.error('Failed to parse message:', e.message);
      }
    }
  });

  client.onClose((err, reason) => {
    console.log(`\nDisconnected: ${reason.code} - ${reason.reason}`);
    process.exit(0);
  });

  client.onError((err, error) => {
    console.error(`Error: ${error}`);
  });

  await client.connect();

  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
    prompt: '> '
  });

  rl.prompt();

  rl.on('line', (line) => {
    const trimmed = line.trim();

    if (trimmed === '/quit') {
      client.close(1000, 'User quit');
      rl.close();
      return;
    }

    if (trimmed === '/stats') {
      client.sendText(JSON.stringify({ type: 'stats' }));
      rl.prompt();
      return;
    }

    if (trimmed.startsWith('/private ')) {
      const parts = trimmed.split(' ');
      if (parts.length < 3) {
        console.log('Usage: /private <userId> <message>');
      } else {
        const targetUserId = parseInt(parts[1]);
        const message = parts.slice(2).join(' ');
        client.sendText(JSON.stringify({
          type: 'private',
          targetUserId,
          message
        }));
      }
      rl.prompt();
      return;
    }

    if (trimmed.length > 0) {
      client.sendText(JSON.stringify({
        type: 'chat',
        message: trimmed
      }));
    }

    rl.prompt();
  });

  rl.on('close', () => {
    if (client.isConnected) {
      client.close(1000, 'User quit');
    }
  });
}

main().catch(console.error);
