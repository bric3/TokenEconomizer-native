use serde_json::{Value, json};
use std::{ffi::OsStr, path::Path};
use tokeneconomizer::dead_code::{self, Analyzer, Item};

pub fn dispatch(op: &str, args: &Value) -> Result<Value, String> {
    let root = Path::new(args["root"].as_str().unwrap_or("."));
    let python = OsStr::new(args["python"].as_str().unwrap_or("python3"));
    if op == "main" {
        let argv: Vec<String> =
            serde_json::from_value(args["argv"].clone()).map_err(|error| error.to_string())?;
        return Ok(match dead_code::run("dead_code.py", &argv, root, python) {
            Ok(output) => {
                json!({"value":output.code,"exit":output.argument_exit,"stdout":output.stdout,"stderr":output.stderr})
            }
            Err(error) => json!({"error":error}),
        });
    }
    if op == "counts" || op == "markdown" {
        let items = args["items"].as_array().ok_or("missing items")?;
        return Ok(
            match if op == "counts" {
                dead_code::counts(items)
            } else {
                dead_code::markdown(items, args["fixed"] == true).map(Value::String)
            } {
                Ok(value) => json!({"value":value}),
                Err(error) => json!({"error":error}),
            },
        );
    }
    let mut analyzer = match Analyzer::new(python) {
        Ok(analyzer) => analyzer,
        Err(error) => return Ok(json!({"error":error})),
    };
    let mut updated = None;
    let result = match op {
        "scan" => analyzer.scan(root),
        "sweep" => analyzer.sweep(root, args["max_passes"].as_i64().unwrap_or(20)),
        "fix" => {
            let mut items: Vec<Item> =
                serde_json::from_value(args["items"].clone()).map_err(|error| error.to_string())?;
            let result = analyzer.fix(root, &mut items);
            updated = Some(items.clone());
            result.map(|()| items)
        }
        _ => return Err(format!("unknown dead-code operation: {op}")),
    };
    Ok(match result {
        Ok(value) => {
            json!({"value":value,"items":updated,"stdout":analyzer.stdout(),"stderr":analyzer.stderr()})
        }
        Err(error) => {
            json!({"error":error,"items":updated,"stdout":analyzer.stdout(),"stderr":analyzer.stderr()})
        }
    })
}
