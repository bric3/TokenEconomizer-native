use super::{Failure, Result, Runtime};
use crate::{
    plugin::paths,
    value::{python_type, scalar_text, truthy},
};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::{fs, path::Path};

pub fn grading_path(explicit: &Value, env: &Map<String, Value>) -> Result<Value> {
    if truthy(explicit) {
        Ok(json!({"path":explicit,"explicit":true}))
    } else {
        default_grading_path(env).map(|path| json!({"path":path,"explicit":false}))
    }
}

pub fn default_grading_path(env: &Map<String, Value>) -> Result<String> {
    if let Some(path) = env
        .get("TE_SWEBENCH_GRADING")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        return Ok(path.into());
    }
    if let Some(data) = env
        .get("DATA")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        return Ok(Path::new(data)
            .join("swebench/swebench_grading.json")
            .to_string_lossy()
            .into_owned());
    }
    Err(Failure::new(
        "RuntimeError",
        "no grading table: set TE_SWEBENCH_GRADING to the instance_id -> grading fields JSON (or DATA to the root holding swebench/swebench_grading.json) -- see the repo's .env.example",
    ))
}

pub fn swebench_repo(env: &Map<String, Value>, cwd: &Path) -> Result<String> {
    let repo = env.get("TE_SWEBENCH_REPO").and_then(Value::as_str).filter(|s| !s.is_empty()).ok_or_else(|| Failure::new("RuntimeError", "no SWE-bench checkout: set TE_SWEBENCH_REPO to one supplying the official grader (swebench.harness.grading) -- see the repo's .env.example"))?;
    paths::realpath(&cwd.join(repo))
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(|e| Failure::new("OSError", e))
}

pub(in crate::experiments) fn parse(text: &str) -> Result<Value> {
    serde_json::from_str(text).map_err(|error| {
        if let Err(crate::python_json::ParseError::Syntax { message, position }) =
            crate::python_json::parse_result(text.as_bytes(), 0)
        {
            let mut failure = Failure::new("JSONDecodeError", message);
            failure.details = Some(Box::new(json!({"doc":text,"pos":position})));
            return failure;
        }
        // shortcut: audit Python-valid numeric and Unicode values that serde cannot represent.
        let display = error.to_string();
        let message = display
            .rsplit_once(" at line ")
            .map_or(display.as_str(), |(message, _)| message);
        let mut failure = Failure::new("JSONDecodeError", message);
        let line_start = text
            .split_inclusive('\n')
            .take(error.line().saturating_sub(1))
            .map(str::len)
            .sum::<usize>();
        let offset = (line_start + error.column().saturating_sub(1)).min(text.len());
        let position = text.as_bytes()[..offset]
            .iter()
            .filter(|byte| (**byte & 0xc0) != 0x80)
            .count();
        failure.details = Some(Box::new(json!({"doc":text,"pos":position})));
        failure
    })
}

pub fn load(path: &Path, cwd: &Path) -> Result<Value> {
    parse(&load_text(path, cwd)?)
}

pub(in crate::experiments) fn load_text(path: &Path, cwd: &Path) -> Result<String> {
    if path.as_os_str().as_encoded_bytes().contains(&0) {
        return Err(Failure::new("ValueError", "embedded null byte"));
    }
    let bytes = fs::read(cwd.join(path)).map_err(|error| Failure::for_path(error, path))?;
    let text = String::from_utf8(bytes).map_err(|error| {
        let utf8 = error.utf8_error();
        let start = utf8.valid_up_to();
        let bytes = error.into_bytes();
        let first = bytes[start];
        let reason = match (first,utf8.error_len()) {
            (0xc2..=0xf4,None) => "unexpected end of data",
            (0xc2..=0xf4,Some(_)) => "invalid continuation byte",
            _ => "invalid start byte",
        };
        let mut failure = Failure::new("UnicodeDecodeError", reason);
        failure.details = Some(Box::new(json!({"bytes":bytes,"start":start,"end":start+utf8.error_len().unwrap_or(bytes.len()-start),"reason":reason})));
        failure
    })?;
    Ok(text.replace("\r\n", "\n").replace('\r', "\n"))
}

fn mapping(value: &Value) -> Result<&Map<String, Value>> {
    value.as_object().ok_or_else(|| {
        Failure::new(
            "AttributeError",
            format!("'{}' object has no attribute 'get'", python_type(value)),
        )
    })
}
fn copy_row(row: &Value) -> Result<Map<String, Value>> {
    if let Some(row) = row.as_object() {
        return Ok(row.clone());
    }
    let Some(pairs) = row.as_array() else {
        if row.as_str() == Some("") {
            return Ok(Map::new());
        }
        if row.is_string() {
            return Err(Failure::new(
                "ValueError",
                "dictionary update sequence element #0 has length 1; 2 is required",
            ));
        }
        return Err(Failure::new(
            "TypeError",
            format!("'{}' object is not iterable", python_type(row)),
        ));
    };
    let mut out = Map::new();
    for (index, pair) in pairs.iter().enumerate() {
        let values = match pair {
            Value::Array(values) => values.clone(),
            Value::String(text) => text.chars().map(|c| json!(c.to_string())).collect(),
            Value::Object(values) => values.keys().map(|key| json!(key)).collect(),
            _ => {
                return Err(Failure::new("TypeError", "object is not iterable"));
            }
        };
        if values.len() != 2 {
            return Err(Failure::new(
                "ValueError",
                format!(
                    "dictionary update sequence element #{index} has length {}; 2 is required",
                    values.len()
                ),
            ));
        }
        // shortcut: grading tables use string keys; extend key types in the JSON parity audit.
        let key = values[0]
            .as_str()
            .ok_or_else(|| Failure::new("TypeError", "grading field names must be strings"))?;
        out.insert(key.into(), values[1].clone());
    }
    Ok(out)
}

#[derive(Deserialize)]
pub struct Grader {
    pub path: String,
    pub rows: Value,
}
impl Grader {
    pub fn instance(&self, instance_id: &str) -> Result<Value> {
        let row = mapping(&self.rows)?
            .get(instance_id)
            .filter(|v| !v.is_null())
            .ok_or_else(|| {
                Failure::new("KeyError", format!("{instance_id} not in {}", self.path))
            })?;
        let mut row = copy_row(row)?;
        row.insert("instance_id".into(), json!(instance_id));
        for key in ["FAIL_TO_PASS", "PASS_TO_PASS"] {
            if let Some(text) = row.get(key).and_then(Value::as_str) {
                row.insert(key.into(), parse(text)?);
            }
        }
        row.entry("problem_statement").or_insert(json!(""));
        Ok(json!(row))
    }
}

pub fn eval_script(instance_id: &Value, runtime: &mut dyn Runtime) -> Result<Value> {
    let instance = runtime.call("grader_instance", json!([instance_id]))?;
    runtime.call("make_test_spec", json!([instance]))?;
    runtime.call("eval_script_value", json!([]))
}
pub fn score(
    instance_id: &Value,
    patch: &Value,
    log_path: &Value,
    runtime: &mut dyn Runtime,
) -> Result<Value> {
    let instance = runtime.call("grader_instance", json!([instance_id]))?;
    runtime.call("make_test_spec", json!([instance]))?;
    let prediction =
        json!({"instance_id":instance_id,"model_patch":patch,"model_name_or_path":"te_harness"});
    let report = runtime.call("get_eval_report", json!([prediction, log_path, true]))?;
    let key = instance_id.as_str().ok_or_else(|| {
        Failure::new(
            "TypeError",
            format!(
                "expected string instance_id, received {}",
                scalar_text(instance_id)
            ),
        )
    })?;
    let row = mapping(&report)?.get(key).cloned().unwrap_or(json!({}));
    let resolved = mapping(&row)?.get("resolved").is_some_and(truthy);
    Ok(json!([resolved, row]))
}
