use super::Result;
use serde_json::{Value, json};
use std::{
    ffi::OsStr,
    io::{self, BufRead, BufReader, BufWriter, Write},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};

pub(super) struct Renderer {
    child: Child,
    input: BufWriter<ChildStdin>,
    output: BufReader<ChildStdout>,
}
impl Renderer {
    pub fn new(python: &OsStr, backend: &str) -> Result<Self> {
        let mut child = Command::new(python)
            .args(["-I", "-u", "-c", include_str!("renderer_io.py"), backend])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        Ok(Self {
            input: BufWriter::new(child.stdin.take().unwrap()),
            output: BufReader::new(child.stdout.take().unwrap()),
            child,
        })
    }
    fn request(&mut self, request: Value) -> Result<Value> {
        serde_json::to_writer(&mut self.input, &request)?;
        self.input.write_all(b"\n")?;
        self.input.flush()?;
        let mut response = String::new();
        if self.output.read_line(&mut response)? == 0 {
            return Err(io::Error::other("Figure renderer binding closed its output").into());
        }
        let response: Value = serde_json::from_str(&response)?;
        if let Some(error) = response["error"].as_str() {
            return Err(io::Error::other(error).into());
        }
        Ok(response["value"].clone())
    }
    pub fn call(&mut self, target: &str, args: Value, kwargs: Value) -> Result<String> {
        let value =
            self.request(json!({"op":"call","target":target,"args":args,"kwargs":kwargs}))?;
        value.as_str().map(str::to_owned).ok_or_else(|| {
            io::Error::other("Figure renderer binding returned an invalid object handle").into()
        })
    }
    pub fn value(&mut self, target: &str) -> Result<Value> {
        self.request(json!({"op":"value","target":target}))
    }
    pub fn keep(&mut self, handles: &[String]) -> Result<()> {
        self.request(json!({"op":"keep","handles":handles}))?;
        Ok(())
    }
}
impl Drop for Renderer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
pub(super) fn object(handle: &str) -> Value {
    json!({"$ref":handle})
}
