use serde_json::{Value, json};
use std::path::Path;
use tokeneconomizer::{
    plugin::adapter::{self, LiveAdapter},
    toolprune::protected::{self, ProtectedTools},
};

fn text<'a>(args: &'a Value, key: &str) -> &'a str {
    args[key].as_str().unwrap_or("")
}
pub(super) fn dispatch(op: &str, args: &Value) -> Result<Value, String> {
    Ok(match op {
        "live_adapter.constants" => {
            json!({"INTERACTIVE_PROTECTED":adapter::INTERACTIVE_PROTECTED,"COPILOT_PROTECTED":adapter::COPILOT_PROTECTED,"HIGH_RISK":adapter::HIGH_RISK,"HIGH_RISK_FACTOR":adapter::HIGH_RISK_FACTOR})
        }
        "live_adapter.detect_vocabulary" => json!(adapter::detect_vocabulary(
            Path::new(text(args, "repo_root")),
            args["preference"].as_str().unwrap_or("auto")
        )),
        "live_adapter.catalog_path" => json!(adapter::catalog_path(
            Path::new(text(args, "state_root")),
            text(args, "host")
        )),
        "live_adapter.load_catalog" => json!(adapter::load_catalog(
            Path::new(text(args, "state_root")),
            text(args, "host")
        )?),
        "live_adapter.protected_contains" => {
            let tools: ProtectedTools =
                serde_json::from_value(args["tools"].clone()).map_err(|e| e.to_string())?;
            json!(tools.contains(text(args, "name")))
        }
        "live_adapter.fnmatchcase" => json!(protected::fnmatchcase(
            text(args, "name"),
            text(args, "pattern")
        )),
        "live_adapter.new" => json!(LiveAdapter::new(
            text(args, "host"),
            Path::new(text(args, "repo_root")),
            args["vocabulary"].as_str().unwrap_or("auto"),
            serde_json::from_value(args["catalog"].clone()).map_err(|e| e.to_string())?,
            serde_json::from_value(args["protect"].clone()).map_err(|e| e.to_string())?
        )),
        _ => {
            let adapter: LiveAdapter =
                serde_json::from_value(args["self"].clone()).map_err(|e| e.to_string())?;
            let path = Path::new(text(args, "path"));
            match op {
                "live_adapter.load_trajectory" => json!(adapter.load_trajectory(path)),
                "live_adapter.tool_usage" => json!(adapter.tool_usage(path)?),
                "live_adapter.parse_catalog" => json!(
                    adapter
                        .parse_catalog_with_source(Path::new(text(args, "run_dir_path")))?
                        .0
                ),
                "live_adapter.parse_catalog_with_source" => {
                    json!(adapter.parse_catalog_with_source(Path::new(text(args, "run_dir_path")))?)
                }
                "live_adapter.protected_tools" => json!(adapter.protected_tools()),
                "live_adapter.inapplicable_tools" => json!(adapter.inapplicable_tools()),
                "live_adapter.hardcoded_exclusions" => json!(
                    adapter.hardcoded_exclusions(
                        &serde_json::from_value::<Vec<String>>(args["used_tools"].clone())
                            .map_err(|e| e.to_string())?
                    )
                ),
                "live_adapter.spiral_snapshot" => adapter.spiral_snapshot(path),
                "live_adapter.problem_statements" => json!(adapter.problem_statements()),
                "live_adapter.env_vocabulary" => json!(adapter.env_vocabulary()),
                _ => return Err(format!("unknown live adapter operation: {op}")),
            }
        }
    })
}
