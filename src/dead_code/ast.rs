use super::{Error, Result};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{HashSet, VecDeque},
    ffi::OsStr,
    io::{BufRead, BufReader, BufWriter, Write},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};

pub(super) struct Session {
    child: Child,
    input: BufWriter<ChildStdin>,
    output: BufReader<ChildStdout>,
    pub stdlib: HashSet<String>,
    pub stdout: String,
    pub stderr: String,
}

impl Session {
    pub fn new(python: &OsStr) -> Result<Self> {
        let mut child = Command::new(python)
            .args(["-I", "-u", "-c", include_str!("ast_io.py")])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let mut session = Self {
            input: BufWriter::new(child.stdin.take().unwrap()),
            output: BufReader::new(child.stdout.take().unwrap()),
            child,
            stdlib: HashSet::new(),
            stdout: String::new(),
            stderr: String::new(),
        };
        session.stdlib = serde_json::from_value(session.call("stdlib", json!([]))?)
            .map_err(|error| Error::new("RuntimeError", error.to_string()))?;
        Ok(session)
    }
    pub fn call(&mut self, op: &str, args: Value) -> Result<Value> {
        serde_json::to_writer(&mut self.input, &json!({"op":op,"args":args}))
            .map_err(|error| Error::new("RuntimeError", error.to_string()))?;
        self.input.write_all(b"\n")?;
        self.input.flush()?;
        let mut line = String::new();
        if self.output.read_line(&mut line)? == 0 {
            return Err(Error::new(
                "RuntimeError",
                "Python AST binding closed its output",
            ));
        }
        let response: Value = serde_json::from_str(&line)
            .map_err(|error| Error::new("RuntimeError", error.to_string()))?;
        self.stdout
            .push_str(response["stdout"].as_str().unwrap_or(""));
        self.stderr
            .push_str(response["stderr"].as_str().unwrap_or(""));
        if let Some(error) = response.get("error") {
            return Err(serde_json::from_value(error.clone())
                .map_err(|error| Error::new("RuntimeError", error.to_string()))?);
        }
        Ok(response["value"].clone())
    }
    pub fn parse(&mut self, source: &str, filename: &str) -> Result<Tree> {
        serde_json::from_value(self.call("parse", json!([source, filename]))?)
            .map_err(|error| Error::new("RuntimeError", error.to_string()))
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Deserialize)]
pub(super) struct Tree {
    pub nodes: Vec<Value>,
}
pub(super) fn reference(value: &Value) -> Option<usize> {
    value
        .get("node")
        .and_then(Value::as_u64)
        .map(|id| id as usize)
}
impl Tree {
    pub fn kind(&self, node: usize) -> &str {
        self.nodes[node]["_type"].as_str().unwrap_or("")
    }
    pub fn text(&self, node: usize, field: &str) -> &str {
        self.nodes[node][field].as_str().unwrap_or("")
    }
    pub fn child(&self, node: usize, field: &str) -> Option<usize> {
        reference(&self.nodes[node][field])
    }
    pub fn list(&self, node: usize, field: &str) -> Vec<usize> {
        self.nodes[node][field]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(reference)
            .collect()
    }
    pub fn span(&self, node: usize) -> (usize, usize) {
        (
            self.nodes[node]["lineno"].as_u64().unwrap_or(0) as usize,
            self.nodes[node]["end_lineno"].as_u64().unwrap_or(0) as usize,
        )
    }
    pub fn walk(&self, node: usize) -> Vec<usize> {
        let mut queue = VecDeque::from([node]);
        let mut nodes = Vec::new();
        while let Some(node) = queue.pop_front() {
            nodes.push(node);
            for value in self.nodes[node].as_object().unwrap().values() {
                if let Some(child) = reference(value) {
                    queue.push_back(child);
                } else if let Some(values) = value.as_array() {
                    queue.extend(values.iter().filter_map(reference));
                }
            }
        }
        nodes
    }
}
