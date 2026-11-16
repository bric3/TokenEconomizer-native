use crate::{
    loopguard::{state::canon_tool, turn::exit_status_covers},
    value::truthy,
};
use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

pub const RESULT_CAP: usize = 20000;
pub static EXIT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)^\s*(?:exit code|process exited with code)[:\s]+(-?\d+)").unwrap()
});

pub fn claude_success_code(
    name: &str,
    args: &Value,
    response: Option<&Value>,
) -> Result<Option<i64>, String> {
    if canon_tool(name) != "bash"
        || response.is_some_and(|resp| resp.is_object() && truthy(&resp["interrupted"]))
    {
        return Ok(None);
    }
    Ok(exit_status_covers(args["command"].as_str().unwrap_or(""))?.then_some(0))
}
