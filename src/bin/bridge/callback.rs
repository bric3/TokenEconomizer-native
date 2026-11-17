use std::{cell::RefCell, io};

use serde_json::{Value, json};
use tokeneconomizer::runner::Runner;

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
        self.pending.get_or_insert(request);
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

    pub(super) fn finish_result(self, value: Result<Value, String>) -> Result<Value, String> {
        self.pending.map(Ok).unwrap_or(value)
    }
}

pub(super) struct RunnerTransport<'a>(pub(super) &'a RefCell<CallbackTape>);
impl Runner for RunnerTransport<'_> {
    fn run(&mut self, prompt: &str, model: &str) -> Result<(String, Option<f64>), String> {
        let reply = self
            .0
            .borrow_mut()
            .call(json!({"callback":"runner","prompt":prompt,"model":model}))?;
        Ok((
            reply["value"][0].as_str().unwrap_or("").into(),
            reply["value"][1].as_f64(),
        ))
    }
    fn estimate_effective(&mut self, prompt: &str, raw: &str) -> Result<f64, String> {
        let reply = self
            .0
            .borrow_mut()
            .call(json!({"callback":"estimate","prompt":prompt,"raw":raw}))?;
        reply["value"]
            .as_f64()
            .ok_or_else(|| "expected numeric runner estimate".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_replies_keep_the_first_callback() {
        let mut tape = CallbackTape::new(&json!({}));
        let first = json!({"callback":"first"});
        assert!(tape.call(first.clone()).is_err());
        assert!(tape.call(json!({"callback":"later"})).is_err());
        assert_eq!(tape.finish(Value::Null), first);
    }
}
