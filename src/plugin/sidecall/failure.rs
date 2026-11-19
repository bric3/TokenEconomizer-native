use super::log;
use crate::value::python_space;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Failure {
    Timeout { timeout: f64 },
    Exit { returncode: i64 },
    Exception { name: String, message: String },
}
impl Failure {
    pub fn from_io(error: &std::io::Error, timeout: f64) -> Self {
        use std::io::ErrorKind;
        if error.kind() == ErrorKind::TimedOut {
            Self::Timeout { timeout }
        } else {
            Self::Exception {
                name: match error.kind() {
                    ErrorKind::NotFound => "FileNotFoundError",
                    ErrorKind::PermissionDenied => "PermissionError",
                    _ => "OSError",
                }
                .into(),
                message: error.to_string(),
            }
        }
    }
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeout { timeout } => {
                write!(f, "side call timed out after {}s", general(*timeout))
            }
            Self::Exit { returncode } => write!(f, "side call exited {returncode}"),
            Self::Exception { message, .. } => f.write_str(message),
        }
    }
}
impl std::error::Error for Failure {}
fn general(value: f64) -> String {
    if !value.is_finite() {
        return value.to_string().to_lowercase();
    }
    let scientific = format!("{value:.5e}");
    let (coefficient, exponent) = scientific.split_once('e').unwrap();
    let exponent: i32 = exponent.parse().unwrap();
    let trim = |s: String| {
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            s
        }
    };
    if (-4..6).contains(&exponent) {
        trim(format!(
            "{value:.precision$}",
            precision = (5 - exponent) as usize
        ))
    } else {
        format!(
            "{}e{}{number:02}",
            trim(coefficient.into()),
            if exponent < 0 { "-" } else { "+" },
            number = exponent.unsigned_abs()
        )
    }
}
pub fn noted<T>(host: &str, root: &Path, result: Result<T, Failure>) -> Result<T, Failure> {
    result.inspect_err(|failure| {
        let note = match failure {
            Failure::Timeout { timeout } => {
                format!("no reply (timed out after {}s)", general(*timeout))
            }
            Failure::Exit { returncode } => format!("no reply (exit {returncode})"),
            Failure::Exception { name, message } => {
                let what: String = message
                    .split(python_space)
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ")
                    .chars()
                    .take(200)
                    .collect();
                format!("failed ({name}: {what})")
            }
        };
        log(root, &format!("sidecall {host}: {note}\n"));
    })
}
