use std::path::Path;

use serde_json::{Map, Value, json};

use super::paths::realpath;
use crate::{
    store::Store,
    value::{scalar_text, truthy},
};

pub const APPLIED: &str = "untool_applied.json";

pub fn canonical(path: &str) -> Result<String, String> {
    if path.is_empty() {
        Ok(String::new())
    } else {
        realpath(Path::new(path)).map(|p| p.to_string_lossy().into_owned())
    }
}
fn entries(value: &Value) -> Vec<Value> {
    match value {
        Value::Array(a) => a.clone(),
        Value::String(s) => s.chars().map(|c| json!(c.to_string())).collect(),
        Value::Object(m) => m.keys().map(|k| json!(k)).collect(),
        _ => Vec::new(),
    }
}
pub fn applied_by_path(doc: &Value) -> Result<Map<String, Value>, String> {
    if let Some(by) = doc["by_path"].as_object() {
        by.iter()
            .map(|(path, tools)| Ok((canonical(path)?, json!(entries(tools)))))
            .collect()
    } else if truthy(&doc["tools"]) {
        Ok([(
            canonical(doc["path"].as_str().unwrap_or(""))?,
            json!(entries(&doc["tools"])),
        )]
        .into_iter()
        .collect())
    } else {
        Ok(Map::new())
    }
}
pub fn kept_by_path(doc: &Value) -> Result<Map<String, Value>, String> {
    doc["kept"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(path, tools)| Ok((canonical(path)?, json!(entries(tools)))))
        .collect()
}
pub fn write_applied(
    store: &Store,
    by: &Map<String, Value>,
    kept: Option<&Map<String, Value>>,
    now: f64,
) -> Result<(), String> {
    let doc = document(store, by, kept, now)?;
    store.write(APPLIED, &doc).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn document(
    store: &Store,
    by: &Map<String, Value>,
    kept: Option<&Map<String, Value>>,
    now: f64,
) -> Result<Value, String> {
    let old = store
        .read(APPLIED, Value::Null)
        .map_err(|e| e.to_string())?;
    let kept = kept
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| kept_by_path(&old))?;
    let sorted = |map: &Map<String, Value>| -> Map<String, Value> {
        map.iter()
            .filter(|(_, v)| truthy(v))
            .map(|(k, v)| {
                let mut values = entries(v);
                values.sort_by_key(scalar_text);
                (k.clone(), json!(values))
            })
            .collect()
    };
    Ok(json!({"by_path":sorted(by),"kept":sorted(&kept),"updated":now}))
}
