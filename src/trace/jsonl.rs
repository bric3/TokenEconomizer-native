use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

use serde_json::Value;

pub struct Lines {
    reader: Option<BufReader<File>>,
    buffer: Vec<u8>,
    skip_lf: bool,
}

pub fn lines(path: &Path) -> Lines {
    Lines {
        reader: File::open(path).ok().map(BufReader::new),
        buffer: Vec::new(),
        skip_lf: false,
    }
}

impl Iterator for Lines {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        loop {
            self.buffer.clear();
            loop {
                let reader = self.reader.as_mut()?;
                let chunk = match reader.fill_buf() {
                    Ok(chunk) if !chunk.is_empty() => chunk,
                    _ => {
                        self.reader = None;
                        break;
                    }
                };
                // Python text files recognise LF, CRLF and bare CR as line endings.
                if self.skip_lf && chunk[0] == b'\n' {
                    reader.consume(1);
                    self.skip_lf = false;
                    continue;
                }
                self.skip_lf = false;
                let end = chunk.iter().position(|b| matches!(b, b'\r' | b'\n'));
                let length = end.unwrap_or(chunk.len());
                self.buffer.extend_from_slice(&chunk[..length]);
                if end.is_some() {
                    self.skip_lf = chunk[length] == b'\r';
                }
                reader.consume(length + usize::from(end.is_some()));
                if end.is_some() {
                    break;
                }
            }
            let text = String::from_utf8_lossy(&self.buffer);
            let text = text.trim();
            if !text.is_empty() {
                return Some(text.to_owned());
            }
            self.reader.as_ref()?;
        }
    }
}

pub fn records(path: &Path) -> impl Iterator<Item = Value> {
    lines(path)
        .filter_map(|line| serde_json::from_str::<Value>(&line).ok())
        .filter(Value::is_object)
}
