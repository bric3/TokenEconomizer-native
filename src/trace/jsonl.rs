use std::{
    fs::File,
    io::{self, BufRead, BufReader, Bytes, Read},
    iter::Peekable,
    path::Path,
};

use crate::value::python_space;
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

impl Lines {
    fn next_with<T>(&mut self, parse: impl FnOnce(&str) -> T) -> Option<T> {
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
            let text = text.trim_matches(python_space);
            if !text.is_empty() {
                return Some(parse(text));
            }
            self.reader.as_ref()?;
        }
    }
}

impl Iterator for Lines {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        self.next_with(str::to_owned)
    }
}

pub fn records(path: &Path) -> impl Iterator<Item = Value> {
    let mut input = lines(path);
    std::iter::from_fn(move || {
        loop {
            let value = input.next_with(|line| serde_json::from_str::<Value>(line).ok())?;
            if let Some(value) = value.filter(Value::is_object) {
                return Some(value);
            }
        }
    })
}

pub struct TextReader {
    bytes: Peekable<Bytes<BufReader<File>>>,
    skip_lf: bool,
}

impl TextReader {
    pub fn open(path: &Path) -> io::Result<Self> {
        Ok(Self {
            bytes: BufReader::new(File::open(path)?).bytes().peekable(),
            skip_lf: false,
        })
    }

    fn read_char(&mut self) -> io::Result<Option<char>> {
        let Some(first) = self.bytes.next().transpose()? else {
            return Ok(None);
        };
        if first < 0x80 {
            return Ok(Some(first as char));
        }
        let width = match first {
            0xc2..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf4 => 4,
            _ => return Ok(Some('\u{fffd}')),
        };
        let mut bytes = [0; 4];
        bytes[0] = first;
        for (index, slot) in bytes.iter_mut().enumerate().take(width).skip(1) {
            let Some(next) = self.bytes.peek() else {
                return Ok(Some('\u{fffd}'));
            };
            let next = match next {
                Ok(next) => *next,
                Err(_) => {
                    return self.bytes.next().unwrap().map(|_| None);
                }
            };
            let valid = (0x80..=0xbf).contains(&next)
                && (index != 1
                    || match first {
                        0xe0 => next >= 0xa0,
                        0xed => next <= 0x9f,
                        0xf0 => next >= 0x90,
                        0xf4 => next <= 0x8f,
                        _ => true,
                    });
            if !valid {
                return Ok(Some('\u{fffd}'));
            }
            *slot = self.bytes.next().unwrap()?;
        }
        Ok(std::str::from_utf8(&bytes[..width])
            .ok()
            .and_then(|s| s.chars().next()))
    }

    pub fn readline(&mut self, limit: usize) -> io::Result<String> {
        let mut out = String::new();
        let mut n = 0;
        while n < limit {
            let Some(c) = self.read_char()? else {
                break;
            };
            if self.skip_lf && c == '\n' {
                self.skip_lf = false;
                continue;
            }
            self.skip_lf = false;
            let c = if c == '\r' {
                self.skip_lf = true;
                '\n'
            } else {
                c
            };
            out.push(c);
            n += 1;
            if c == '\n' {
                break;
            }
        }
        Ok(out)
    }
}
