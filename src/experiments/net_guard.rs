use crate::value::truthy;
use regex::Regex;
use serde_json::{Value, json};
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    path::Path,
    sync::LazyLock,
};
mod input;
mod install;
pub use install::{install_entry, install_hook};

pub const ANSWER_KEY_HOSTS: &[&str] = &[
    "github.com",
    "www.github.com",
    "api.github.com",
    "codeload.github.com",
    "raw.githubusercontent.com",
    "objects.githubusercontent.com",
    "patch-diff.githubusercontent.com",
    "gist.githubusercontent.com",
];
pub const HOSTS_MARKER: &str = "# TokenEconomizer answer-key guard";
pub const LOG_ENV: &str = "TE_NET_GUARD_LOG";
pub const REASON: &str = "Blocked: this benchmark's grading specs are public on GitHub, so fetching from it would read the answer key rather than solve the task. Work from the files in the workspace.";

pub fn constants() -> Value {
    json!({"ANSWER_KEY_HOSTS":ANSWER_KEY_HOSTS,"HOSTS_MARKER":HOSTS_MARKER,"LOG_ENV":LOG_ENV,"REASON":REASON})
}

pub fn hosts_block() -> String {
    let mut out = format!("{HOSTS_MARKER}\n");
    for host in ANSWER_KEY_HOSTS {
        out.push_str(&format!("127.0.0.1 {host}\n"));
    }
    out
}
pub fn blocked(command: &str) -> bool {
    // Python re.I also matches dotted and dotless I against the ASCII i in github.
    static DENY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i:g)[iIİı](?i:thub)").unwrap());
    DENY.is_match(command)
}
pub fn decide(command: &Value) -> Result<(bool, &'static str), String> {
    if !truthy(command) {
        return Ok((false, ""));
    }
    let command = command.as_str().ok_or_else(|| {
        format!(
            "TypeError: expected string or bytes-like object, got '{}'",
            crate::value::python_type(command)
        )
    })?;
    Ok(if blocked(command) {
        (true, REASON)
    } else {
        (false, "")
    })
}
pub fn as_command(value: &Value) -> String {
    input::Node::from(value).as_command().lossy()
}
pub fn find_command(node: &Value, depth: i64) -> String {
    input::Node::from(node).find_command(depth).lossy()
}
pub fn hook_command(payload: &Value) -> String {
    input::Node::from(payload).hook_command().lossy()
}
pub fn lifted(value: &str) -> bool {
    ["1", "true", "yes"].contains(
        &value.trim_matches(|c: char| c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}')),
    )
}
pub fn run(
    input: &mut dyn Read,
    error: &mut dyn Write,
    allow: &str,
    log_path: Option<&Path>,
    int_max_digits: usize,
) -> i32 {
    if lifted(allow) {
        return 0;
    }
    let mut bytes = Vec::new();
    if input.read_to_end(&mut bytes).is_err() {
        return 0;
    }
    let Some(command) = input::command(&bytes, int_max_digits) else {
        return 0;
    };
    if !blocked(&command.lossy()) {
        return 0;
    }
    if let Some(path) = log_path.filter(|p| !p.as_os_str().is_empty())
        && let Ok(mut file) = OpenOptions::new().append(true).create(true).open(path)
    {
        let record = command.record();
        let _ = file.write_all(record.as_bytes());
    }
    // A telemetry write failure must preserve the refusal.
    let _ = writeln!(error, "{REASON}");
    2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifted_guard_does_not_read_and_input_errors_allow_the_call() {
        struct Input {
            reads: usize,
        }
        impl Read for Input {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                self.reads += 1;
                Err(std::io::Error::other("fixture read failure"))
            }
        }
        let mut input = Input { reads: 0 };
        let mut errors = Vec::new();
        assert_eq!(run(&mut input, &mut errors, " yes ", None, 4300), 0);
        assert_eq!(input.reads, 0);
        assert_eq!(run(&mut input, &mut errors, "", None, 4300), 0);
        assert_eq!(input.reads, 1);
        assert!(errors.is_empty());
    }
}
