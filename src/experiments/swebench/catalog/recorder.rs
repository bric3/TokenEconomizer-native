//! Recorder state and HTTP forwarding decisions. Socket and TLS operations stay in the SDK.
use crate::{
    python_json::{self, Node, Text},
    value::json_text,
};
use indexmap::IndexMap;
use serde_json::{Value, json};
use std::{
    fs,
    io::{self, BufRead, Write},
    path::Path,
};

#[derive(Default)]
pub struct Recorder {
    tools: IndexMap<Text, Text>,
}
impl Recorder {
    fn offer(&mut self, document: &Value, out: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let body = python_json::parse(document.as_str().ok_or("invalid JSON")?.as_bytes(), 0)
            .ok_or("invalid JSON")?;
        if !matches!(body, Node::Dict(_)) {
            return Err("not an object".into());
        }
        let absent = Node::Null;
        let tools = body.field("tools").unwrap_or(&absent);
        let mut got = IndexMap::new();
        if tools.truthy() {
            if let Node::List(tools) = tools {
                for tool in tools {
                    if !matches!(tool, Node::Dict(_))
                        || !tool.field("name").is_some_and(Node::truthy)
                    {
                        continue;
                    }
                    let Some(Node::String(name)) = tool.field("name") else {
                        return Err("tool name must be a string".into());
                    };
                    let description = tool.field("description").unwrap_or(&absent);
                    let points = if !description.truthy() {
                        &[][..]
                    } else if let Node::String(description) = description {
                        description.0.as_slice()
                    } else {
                        return Err("description must be text".into());
                    };
                    let space = |point: &u32| {
                        char::from_u32(*point)
                            .is_some_and(|c| c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}'))
                    };
                    let start = points
                        .iter()
                        .position(|point| !space(point))
                        .unwrap_or(points.len());
                    let end = points
                        .iter()
                        .rposition(|point| !space(point))
                        .map_or(start, |index| index + 1);
                    got.insert(name.clone(), Text(points[start..end].to_vec()));
                }
            } else if !matches!(tools, Node::String(_) | Node::Dict(_)) {
                return Err("tools must be iterable".into());
            }
        }
        let mut changed = false;
        for (name, description) in got {
            if description.0.len() > self.tools.get(&name).map_or(0, |value| value.0.len()) {
                self.tools.insert(name, description);
                changed = true;
            }
        }
        if changed {
            fs::write(out, self.snapshot())?;
        }
        Ok(())
    }
    fn snapshot(&self) -> String {
        Node::Dict(
            self.tools
                .iter()
                .map(|(name, description)| (name.clone(), Node::String(description.clone())))
                .collect(),
        )
        .json(false)
    }
    pub fn dispatch(&mut self, request: &Value, out: &Path, upstream: &str, key: &str) -> Value {
        match request["op"].as_str().unwrap_or("") {
            "request" => {
                if request["method"] == "POST" {
                    // A malformed offer must not prevent the original HTTP request from reaching the API.
                    let _ = self.offer(&request["document"], out);
                }
                let mut headers = Vec::new();
                let mut version = None;
                if let Some(incoming) = request["headers"].as_array() {
                    for header in incoming {
                        let name = header[0].as_str().unwrap_or("").to_ascii_lowercase();
                        if name == "anthropic-version" && version.is_none() {
                            version = Some(header[1].clone());
                        }
                        if ![
                            "host",
                            "content-length",
                            "authorization",
                            "x-api-key",
                            "accept-encoding",
                            "connection",
                        ]
                        .contains(&name.as_str())
                        {
                            headers.push(header.clone());
                        }
                    }
                }
                headers.push(json!(["x-api-key", key]));
                headers.push(json!([
                    "anthropic-version",
                    version.unwrap_or(json!("2023-06-01"))
                ]));
                json!({"url":format!("{}{}",upstream.trim_end_matches('/'),request["path"].as_str().unwrap_or("")),"headers":headers})
            }
            "response" => {
                let mut headers: Vec<Value> = request["headers"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|header| {
                        ![
                            "transfer-encoding",
                            "content-length",
                            "connection",
                            "content-encoding",
                        ]
                        .contains(
                            &header[0]
                                .as_str()
                                .unwrap_or("")
                                .to_ascii_lowercase()
                                .as_str(),
                        )
                    })
                    .cloned()
                    .collect();
                headers.push(json!(["transfer-encoding", "chunked"]));
                json!(headers)
            }
            "error" => {
                let (status, body) = if let Some(status) = request.get("status") {
                    (status.clone(), request["body"].clone())
                } else {
                    (
                        json!(502),
                        json!(json_text(&json!({"error":request["message"]})).into_bytes()),
                    )
                };
                json!({"status":status,"body":body,"headers":[["content-type","application/json"],["content-length",body.as_array().map_or(0,Vec::len).to_string()]]})
            }
            _ => json!({"error":"unknown catalog recorder operation"}),
        }
    }
}

pub fn serve(
    input: impl BufRead,
    mut output: impl Write,
    out: &Path,
    upstream: &str,
    key: &str,
) -> io::Result<()> {
    let mut recorder = Recorder::default();
    for line in input.lines() {
        let request: Value = serde_json::from_str(&line?).map_err(io::Error::other)?;
        let value = recorder.dispatch(&request, out, upstream, key);
        serde_json::to_writer(&mut output, &value)?;
        output.write_all(b"\n")?;
        output.flush()?;
    }
    Ok(())
}
