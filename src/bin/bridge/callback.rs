use std::io;

use serde_json::Value;

pub(super) struct CallbackTape {
    replies: Vec<Value>,
    index: usize,
    pending: Option<Value>,
}

impl CallbackTape {
    pub(super) fn new(args: &Value) -> Self {
        Self {
            replies: args["replies"].as_array().cloned().unwrap_or_default(),
            index: 0,
            pending: None,
        }
    }

    pub(super) fn call(&mut self, request: Value) -> Result<Value, String> {
        if let Some(reply) = self.replies.get(self.index) {
            self.index += 1;
            if let Some(error) = reply["error"].as_str() {
                return Err(error.into());
            }
            return Ok(reply.clone());
        }
        self.pending = Some(request);
        Err("callback pending".into())
    }

    pub(super) fn io_call(&mut self, request: Value) -> io::Result<Value> {
        let kind = match self
            .replies
            .get(self.index)
            .and_then(|r| r["error_kind"].as_str())
        {
            Some("PermissionError") => io::ErrorKind::PermissionDenied,
            Some("FileNotFoundError") => io::ErrorKind::NotFound,
            Some("UnicodeDecodeError") => io::ErrorKind::InvalidData,
            _ => io::ErrorKind::Other,
        };
        self.call(request)
            .map_err(|error| io::Error::new(kind, error))
    }

    pub(super) fn finish(self, value: Value) -> Value {
        self.pending.unwrap_or(value)
    }
}
