//! WebSocket message types with zero-copy support

use bytes::Bytes;
use napi::bindgen_prelude::*;
use napi_derive::napi;

/// WebSocket message type
#[napi]
pub enum MessageType {
    /// UTF-8 text message
    Text,
    /// Binary data message
    Binary,
    /// Ping control frame
    Ping,
    /// Pong control frame
    Pong,
    /// Close control frame
    Close,
}

/// WebSocket message wrapper for efficient data transfer
#[napi]
pub struct Message {
    inner: MessageInner,
}

enum MessageInner {
    Text(Bytes),
    Binary(Bytes),
    Ping(Bytes),
    Pong(Bytes),
    Close(Option<crate::error::CloseReason>),
}

#[napi]
impl Message {
    /// Create a text message from a string
    ///
    /// @param text - The text content
    /// @returns A new text message
    ///
    /// @example
    /// ```javascript
    /// const msg = Message.text("Hello, World!");
    /// await ws.send(msg);
    /// ```
    #[napi(factory)]
    pub fn text(text: String) -> Self {
        Self {
            inner: MessageInner::Text(Bytes::from(text)),
        }
    }

    /// Create a binary message from a Buffer
    ///
    /// @param data - The binary data
    /// @returns A new binary message
    ///
    /// @example
    /// ```javascript
    /// const msg = Message.binary(Buffer.from([0x01, 0x02, 0x03]));
    /// await ws.send(msg);
    /// ```
    #[napi(factory)]
    pub fn binary(data: Buffer) -> Self {
        Self {
            inner: MessageInner::Binary(Bytes::from(data.to_vec())),
        }
    }

    /// Create a ping message
    ///
    /// @param data - Optional ping payload
    /// @returns A new ping message
    #[napi(factory)]
    pub fn ping(data: Option<Buffer>) -> Self {
        let bytes = data
            .map(|b| Bytes::from(b.to_vec()))
            .unwrap_or_else(Bytes::new);
        Self {
            inner: MessageInner::Ping(bytes),
        }
    }

    /// Create a pong message
    ///
    /// @param data - Optional pong payload
    /// @returns A new pong message
    #[napi(factory)]
    pub fn pong(data: Option<Buffer>) -> Self {
        let bytes = data
            .map(|b| Bytes::from(b.to_vec()))
            .unwrap_or_else(Bytes::new);
        Self {
            inner: MessageInner::Pong(bytes),
        }
    }

    /// Create a close message
    ///
    /// @param code - Close code (default: 1000 for normal closure)
    /// @param reason - Optional close reason string
    /// @returns A new close message
    ///
    /// @example
    /// ```javascript
    /// const msg = Message.close(1000, "Goodbye!");
    /// await ws.send(msg);
    /// ```
    #[napi(factory)]
    pub fn close(code: Option<u32>, reason: Option<String>) -> Self {
        let close_reason = code.map(|c| crate::error::CloseReason {
            code: c as u16,
            reason: reason.unwrap_or_default(),
        });
        Self {
            inner: MessageInner::Close(close_reason),
        }
    }

    /// Get the message type
    #[napi(getter)]
    pub fn message_type(&self) -> MessageType {
        match &self.inner {
            MessageInner::Text(_) => MessageType::Text,
            MessageInner::Binary(_) => MessageType::Binary,
            MessageInner::Ping(_) => MessageType::Ping,
            MessageInner::Pong(_) => MessageType::Pong,
            MessageInner::Close(_) => MessageType::Close,
        }
    }

    /// Check if this is a text message
    #[napi(getter)]
    pub fn is_text(&self) -> bool {
        matches!(self.inner, MessageInner::Text(_))
    }

    /// Check if this is a binary message
    #[napi(getter)]
    pub fn is_binary(&self) -> bool {
        matches!(self.inner, MessageInner::Binary(_))
    }

    /// Check if this is a ping message
    #[napi(getter)]
    pub fn is_ping(&self) -> bool {
        matches!(self.inner, MessageInner::Ping(_))
    }

    /// Check if this is a pong message
    #[napi(getter)]
    pub fn is_pong(&self) -> bool {
        matches!(self.inner, MessageInner::Pong(_))
    }

    /// Check if this is a close message
    #[napi(getter)]
    pub fn is_close(&self) -> bool {
        matches!(self.inner, MessageInner::Close(_))
    }

    /// Check if this is a control frame (ping, pong, or close)
    #[napi(getter)]
    pub fn is_control(&self) -> bool {
        matches!(
            self.inner,
            MessageInner::Ping(_) | MessageInner::Pong(_) | MessageInner::Close(_)
        )
    }

    /// Get the text content (returns null for non-text messages)
    ///
    /// @returns The text content or null
    #[napi]
    pub fn as_text(&self) -> Option<String> {
        match &self.inner {
            MessageInner::Text(b) => {
                // SAFETY: Text messages are UTF-8 validated during parsing
                Some(unsafe { String::from_utf8_unchecked(b.to_vec()) })
            }
            _ => None,
        }
    }

    /// Get the binary data as a Buffer
    ///
    /// Works for text, binary, ping, and pong messages.
    /// Returns null for close messages.
    ///
    /// @returns Buffer containing the message data
    #[napi]
    pub fn as_buffer(&self) -> Option<Buffer> {
        match &self.inner {
            MessageInner::Text(b)
            | MessageInner::Binary(b)
            | MessageInner::Ping(b)
            | MessageInner::Pong(b) => Some(Buffer::from(b.to_vec())),
            MessageInner::Close(_) => None,
        }
    }

    /// Get the close reason (only for close messages)
    ///
    /// @returns The close reason or null
    #[napi]
    pub fn close_reason(&self) -> Option<crate::error::CloseReason> {
        match &self.inner {
            MessageInner::Close(reason) => reason.clone(),
            _ => None,
        }
    }

    /// Get the message payload size in bytes
    #[napi(getter)]
    pub fn size(&self) -> u32 {
        match &self.inner {
            MessageInner::Text(b)
            | MessageInner::Binary(b)
            | MessageInner::Ping(b)
            | MessageInner::Pong(b) => b.len() as u32,
            MessageInner::Close(reason) => {
                reason.as_ref().map(|r| 2 + r.reason.len()).unwrap_or(0) as u32
            }
        }
    }
}

impl From<sockudo_ws::Message> for Message {
    fn from(msg: sockudo_ws::Message) -> Self {
        match msg {
            sockudo_ws::Message::Text(b) => Self {
                inner: MessageInner::Text(b),
            },
            sockudo_ws::Message::Binary(b) => Self {
                inner: MessageInner::Binary(b),
            },
            sockudo_ws::Message::Ping(b) => Self {
                inner: MessageInner::Ping(b),
            },
            sockudo_ws::Message::Pong(b) => Self {
                inner: MessageInner::Pong(b),
            },
            sockudo_ws::Message::Close(reason) => Self {
                inner: MessageInner::Close(reason.map(|r| r.into())),
            },
        }
    }
}

impl From<&Message> for sockudo_ws::Message {
    fn from(msg: &Message) -> Self {
        match &msg.inner {
            MessageInner::Text(b) => sockudo_ws::Message::Text(b.clone()),
            MessageInner::Binary(b) => sockudo_ws::Message::Binary(b.clone()),
            MessageInner::Ping(b) => sockudo_ws::Message::Ping(b.clone()),
            MessageInner::Pong(b) => sockudo_ws::Message::Pong(b.clone()),
            MessageInner::Close(reason) => {
                sockudo_ws::Message::Close(reason.clone().map(|r| r.into()))
            }
        }
    }
}
