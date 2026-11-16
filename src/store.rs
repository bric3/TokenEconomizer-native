use std::{
    fs::{self, File, OpenOptions},
    io::{self, ErrorKind, Write},
    path::{Component, Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use serde_json::Value;

use crate::value::{json_pretty, json_text};

pub const REPLACE_BACKOFF: [f64; 5] = [0.01, 0.02, 0.05, 0.1, 0.2];

pub fn retrying<T>(windows: bool, mut call: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    if windows {
        for delay in REPLACE_BACKOFF {
            match call() {
                Err(error) if error.kind() == ErrorKind::PermissionDenied => {
                    thread::sleep(Duration::from_secs_f64(delay))
                }
                result => return result,
            }
        }
    }
    call()
}

pub fn read_document(
    path: &Path,
    default: Value,
    windows: bool,
    mut read: impl FnMut(&Path) -> io::Result<Vec<u8>>,
) -> io::Result<Value> {
    match retrying(windows, || read(path)) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes).unwrap_or(default)),
        Err(error) if matches!(error.kind(), ErrorKind::NotFound | ErrorKind::InvalidData) => {
            Ok(default)
        }
        Err(error) => Err(error),
    }
}

pub fn write_json_atomic(path: &Path, value: &Value, sort_keys: bool) -> io::Result<()> {
    write_json_atomic_with(path, value, sort_keys, cfg!(windows), |src, dst| {
        fs::rename(src, dst)
    })
}

pub fn write_json_atomic_with(
    path: &Path,
    value: &Value,
    sort_keys: bool,
    windows: bool,
    mut replace: impl FnMut(&Path, &Path) -> io::Result<()>,
) -> io::Result<()> {
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(format!(".{}.tmp", std::process::id()));
    let temporary = PathBuf::from(temporary);
    let result = (|| {
        let mut file = File::create(&temporary)?;
        file.write_all(json_pretty(value, sort_keys).as_bytes())?;
        drop(file);
        retrying(windows, || replace(&temporary, path))
    })();
    match fs::remove_file(&temporary) {
        Ok(()) => result,
        Err(error) if error.kind() == ErrorKind::NotFound => result,
        Err(error) => Err(error),
    }
}

#[derive(Clone, Debug)]
pub struct Store {
    pub base: PathBuf,
}

impl Store {
    pub fn new(base: &Path) -> io::Result<Self> {
        let absolute = std::path::absolute(base)?;
        let mut base = PathBuf::new();
        for component in absolute.components() {
            if component == Component::ParentDir {
                base.pop();
            } else {
                base.push(component);
            }
        }
        Ok(Self { base })
    }

    pub fn path(&self, parts: &[impl AsRef<Path>]) -> PathBuf {
        let mut path = self.base.clone();
        for part in parts {
            path.push(part);
        }
        path
    }

    pub fn ensure(&self, parts: &[impl AsRef<Path>]) -> io::Result<PathBuf> {
        let path = self.path(parts);
        fs::create_dir_all(&path)?;
        Ok(path)
    }

    pub fn read(&self, name: &str, default: Value) -> io::Result<Value> {
        read_document(&self.base.join(name), default, cfg!(windows), |path| {
            fs::read(path)
        })
    }

    pub fn write(&self, name: &str, value: &Value) -> io::Result<PathBuf> {
        let target = self.base.join(name);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        write_json_atomic(&target, value, true)?;
        Ok(target)
    }

    pub fn append(&self, name: &str, record: &Value) -> io::Result<()> {
        let target = self.base.join(name);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut record = record.clone();
        record.sort_all_objects();
        let line = json_text(&record) + "\n";
        OpenOptions::new()
            .append(true)
            .create(true)
            .open(target)?
            .write_all(line.as_bytes())
    }

    pub fn read_lines(&self, name: &str, limit: Option<i64>) -> io::Result<Vec<Value>> {
        let text = match fs::read_to_string(self.base.join(name)) {
            Ok(text) => text,
            Err(error) if error.kind() == ErrorKind::InvalidData => return Err(error),
            Err(_) => return Ok(Vec::new()),
        };
        let mut records: Vec<_> = text
            .split(['\r', '\n'])
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter(Value::is_object)
            .collect();
        let start = match limit {
            Some(n) if n > 0 => records.len().saturating_sub(n as usize),
            Some(n) if n < 0 => n.unsigned_abs().min(records.len() as u64) as usize,
            _ => 0,
        };
        records.drain(..start);
        Ok(records)
    }

    pub fn locked(&self, name: &str, timeout: f64) -> io::Result<StoreLock> {
        fs::create_dir_all(&self.base)?;
        let file = OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .open(self.base.join(format!(".{name}.lock")))?;
        let started = Instant::now();
        let acquired = loop {
            if file.try_lock().is_ok() {
                break true;
            }
            if started.elapsed().as_secs_f64() >= timeout {
                break false;
            }
            thread::sleep(Duration::from_millis(50));
        };
        Ok(StoreLock { file, acquired })
    }
}

pub struct StoreLock {
    file: File,
    pub acquired: bool,
}

impl StoreLock {
    pub fn release(self) -> io::Result<()> {
        if self.acquired {
            self.file.unlock()
        } else {
            Ok(())
        }
    }
}
