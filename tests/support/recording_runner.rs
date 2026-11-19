use std::{cell::RefCell, rc::Rc};
use tokeneconomizer::runner::Runner;

pub struct RecordingRunner {
    pub prompts: Rc<RefCell<Vec<String>>>,
    payload: String,
    tokens: f64,
}

impl RecordingRunner {
    pub fn new(payload: &str, tokens: f64) -> Self {
        Self {
            prompts: Rc::default(),
            payload: payload.into(),
            tokens,
        }
    }
}

impl Runner for RecordingRunner {
    fn run(&mut self, prompt: &str, _model: &str) -> Result<(String, Option<f64>), String> {
        self.prompts.borrow_mut().push(prompt.into());
        Ok((self.payload.clone(), Some(self.tokens)))
    }
}
