use serde::{Deserialize, Serialize};

/// RPC Request Envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    pub id: serde_json::Value,
    pub action: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

/// RPC Response Envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    pub id: serde_json::Value,
    pub code: i32,
    pub msg: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl RpcResponse {
    pub fn success(id: serde_json::Value, data: serde_json::Value) -> Self {
        Self {
            id,
            code: 0,
            msg: "OK".to_string(),
            data: Some(data),
        }
    }

    pub fn ok(id: serde_json::Value, msg: impl Into<String>) -> Self {
        Self {
            id,
            code: 0,
            msg: msg.into(),
            data: None,
        }
    }

    pub fn error(id: serde_json::Value, code: i32, msg: impl Into<String>) -> Self {
        Self {
            id,
            code,
            msg: msg.into(),
            data: None,
        }
    }
}

/// Unsolicited Server Push Event Frame
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventFrame {
    pub event: String,
    pub timestamp: u64,
    pub data: serde_json::Value,
}

impl EventFrame {
    pub fn new(event: impl Into<String>, data: serde_json::Value) -> Self {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            event: event.into(),
            timestamp: ts,
            data,
        }
    }
}
