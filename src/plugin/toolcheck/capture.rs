use crate::value::{json_text, truthy};
use serde_json::{Value, json};
use std::{
    io::{self, BufRead, BufReader, Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub(super) struct Capture {
    pub port: u16,
    captured: Arc<Mutex<Vec<Value>>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl Capture {
    pub fn start() -> io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let port = listener.local_addr()?.port();
        listener.set_nonblocking(true)?;
        let captured = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (requests, stopping) = (captured.clone(), stop.clone());
        let worker = thread::spawn(move || {
            while !stopping.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let requests = requests.clone();
                        thread::spawn(move || {
                            // macOS accepted sockets inherit the listener's non-blocking mode.
                            let _ = stream
                                .set_nonblocking(false)
                                .and_then(|()| serve(&mut stream, &requests));
                            let _ = stream.shutdown(Shutdown::Write);
                        });
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10))
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            port,
            captured,
            stop,
            worker: Some(worker),
        })
    }
    pub fn finish(mut self) -> Value {
        self.shutdown();
        self.captured
            .lock()
            .unwrap()
            .first()
            .cloned()
            .unwrap_or(Value::Null)
    }
    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn header_line(reader: &mut impl BufRead) -> io::Result<String> {
    let mut bytes = Vec::new();
    reader.take(65537).read_until(b'\n', &mut bytes)?;
    if bytes.len() > 65536 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "HTTP header line too long",
        ));
    }
    Ok(bytes.into_iter().map(char::from).collect())
}
fn serve(stream: &mut TcpStream, captured: &Mutex<Vec<Value>>) -> io::Result<()> {
    let mut reader = BufReader::new(stream);
    let line = match header_line(&mut reader) {
        Ok(line) => line,
        Err(error) if error.kind() == io::ErrorKind::InvalidData => {
            return reject(reader.get_mut(), "414 Request-URI Too Long");
        }
        Err(error) => return Err(error),
    };
    let method = line.split_whitespace().next().unwrap_or("").to_string();
    let mut length = None;
    let mut headers_done = false;
    // Preserve the standard library server's header limits and first-header selection.
    for _ in 0..=100 {
        let line = match header_line(&mut reader) {
            Ok(line) => line,
            Err(error) if error.kind() == io::ErrorKind::InvalidData => {
                return reject(reader.get_mut(), "431 Line too long");
            }
            Err(error) => return Err(error),
        };
        if line.is_empty() || line == "\r\n" || line == "\n" {
            headers_done = true;
            break;
        }
        if let Some((key, value)) = line.split_once(':')
            && key.eq_ignore_ascii_case("content-length")
            && length.is_none()
        {
            length = Some(value.trim().to_string());
        }
    }
    if !headers_done {
        return reject(reader.get_mut(), "431 Too many headers");
    }
    let (text, content_type) = match method.as_str() {
        "GET" => (
            json_text(&json!({"object":"list","data":[{"id":"gpt-5.4","object":"model"}]})),
            "application/json",
        ),
        "POST" => {
            let length = length
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or("0")
                .parse::<usize>()
                .map_err(io::Error::other)?;
            let mut bytes = Vec::new();
            reader
                .by_ref()
                .take(length as u64)
                .read_to_end(&mut bytes)?;
            let body: Value = serde_json::from_slice(&bytes).unwrap_or(json!({}));
            captured.lock().unwrap().push(body.clone());
            let body = body
                .as_object()
                .ok_or_else(|| io::Error::other("request must be an object"))?;
            let message = json!({"role":"assistant","content":"OK"});
            let usage = json!({"prompt_tokens":1,"completion_tokens":1,"total_tokens":2});
            if body.get("stream").is_some_and(truthy) {
                let chunks = [
                    json!({"choices":[{"index":0,"delta":message,"finish_reason":null}],"object":"chat.completion.chunk"}),
                    json!({"choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"object":"chat.completion.chunk"}),
                    json!({"choices":[],"usage":usage,"object":"chat.completion.chunk"}),
                ];
                let mut text = String::new();
                for chunk in chunks {
                    text.push_str(&format!("data: {}\n\n", json_text(&chunk)));
                }
                text.push_str("data: [DONE]\n\n");
                (text, "text/event-stream")
            } else {
                (
                    json_text(
                        &json!({"object":"chat.completion","usage":usage,"choices":[{"index":0,"message":message,"finish_reason":"stop"}]}),
                    ),
                    "application/json",
                )
            }
        }
        _ => {
            reader
                .get_mut()
                .write_all(b"HTTP/1.0 501 Not Implemented\r\nContent-Length: 0\r\n\r\n")?;
            return Ok(());
        }
    };
    let stream = reader.get_mut();
    write!(
        stream,
        "HTTP/1.0 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n",
        text.len()
    )?;
    stream.write_all(text.as_bytes())
}
fn reject(stream: &mut impl Write, status: &str) -> io::Result<()> {
    write!(stream, "HTTP/1.0 {status}\r\nContent-Length: 0\r\n\r\n")
}
