//! Error types for WebSocket bindings

use napi::bindgen_prelude::*;
use napi_derive::napi;

/// WebSocket error codes matching RFC 6455
#[napi]
pub enum CloseCode {
    /// Normal closure (1000)
    Normal = 1000,
    /// Going away (1001)
    GoingAway = 1001,
    /// Protocol error (1002)
    ProtocolError = 1002,
    /// Unsupported data (1003)
    Unsupported = 1003,
    /// No status received (1005)
    NoStatus = 1005,
    /// Abnormal closure (1006)
    Abnormal = 1006,
    /// Invalid payload (1007)
    InvalidPayload = 1007,
    /// Policy violation (1008)
    PolicyViolation = 1008,
    /// Message too big (1009)
    MessageTooBig = 1009,
    /// Missing extension (1010)
    MissingExtension = 1010,
    /// Internal error (1011)
    InternalError = 1011,
}

/// Close reason containing code and optional message
#[napi(object)]
#[derive(Clone, Debug)]
pub struct CloseReason {
    /// Close code (RFC 6455)
    pub code: u16,
    /// Optional reason string
    pub reason: String,
}

impl From<sockudo_ws::error::CloseReason> for CloseReason {
    fn from(r: sockudo_ws::error::CloseReason) -> Self {
        Self {
            code: r.code,
            reason: r.reason,
        }
    }
}

impl From<CloseReason> for sockudo_ws::error::CloseReason {
    fn from(r: CloseReason) -> Self {
        sockudo_ws::error::CloseReason::new(r.code, r.reason)
    }
}

/// Convert sockudo-ws errors to NAPI errors
#[allow(dead_code)]
pub(crate) fn to_napi_error(e: sockudo_ws::Error) -> Error {
    let msg = e.to_string();
    let status = match &e {
        sockudo_ws::Error::ConnectionClosed => Status::Cancelled,
        sockudo_ws::Error::ConnectionReset => Status::Cancelled,
        sockudo_ws::Error::WouldBlock => Status::WouldDeadlock,
        sockudo_ws::Error::BufferFull => Status::QueueFull,
        sockudo_ws::Error::MessageTooLarge => Status::InvalidArg,
        sockudo_ws::Error::FrameTooLarge => Status::InvalidArg,
        sockudo_ws::Error::InvalidUtf8 => Status::StringExpected,
        sockudo_ws::Error::Protocol(_) => Status::InvalidArg,
        sockudo_ws::Error::HandshakeFailed(_) => Status::GenericFailure,
        _ => Status::GenericFailure,
    };
    Error::new(status, msg)
}
