/* eslint-disable */
/* prettier-ignore */

import binding from './index.js'

export const {
  // Runtime
  initRuntime,
  getWorkerThreads,
  getAvailableCores,

  // Message
  Message,
  MessageType,

  // Config
  Config,
  Compression,
  hftConfig,
  throughputConfig,
  uwsConfig,

  // Error types
  CloseCode,
  CloseReason,

  // Server
  WebSocketServer,
  ServerOptions,
  ConnectionInfo,
  ServerStats,

  // Client
  WebSocketClient,
  ClientOptions,
  connect,

  // WebSocket
  WebSocket,
  ConnectionStats,
} = binding
