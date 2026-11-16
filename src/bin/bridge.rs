use std::io::{self, BufRead, Write};

use serde_json::{Value, json};
use tokeneconomizer::{accounting, ledger, lifecycle::Lifecycle, names, pricing};

fn dispatch(request: &Value) -> Result<Value, String> {
    let args = &request["args"];
    let result = match request["op"].as_str().unwrap_or("") {
        "pricing.resolve_model" => json!(pricing::resolve_model(&args["rec"])),
        "pricing.weights_for" => json!(pricing::weights_for(&args["rec"])?),
        "pricing.has_breakdown" => json!(pricing::has_breakdown(&args["rec"])),
        "pricing.cache_write_1h_tokens" => json!(pricing::cache_write_1h_tokens(&args["usage"])),
        "pricing.effective_tokens" => {
            let weights = if args["weights"].is_null() {
                None
            } else {
                Some(serde_json::from_value(args["weights"].clone()).map_err(|e| e.to_string())?)
            };
            json!(pricing::effective_tokens(&args["rec"], weights)?)
        }
        "pricing.approx_tokens" => json!(pricing::approx_tokens(
            args["text"].as_str().ok_or("expected text")?
        )),
        "ledger.overhead" => json!(ledger::overhead(&args["row"])),
        "ledger.effective_exec" => json!(ledger::effective_exec(&args["row"])?),
        "accounting.effective" => json!(accounting::effective(&args["usage"])),
        "accounting.overhead_total" => json!(accounting::overhead_total(&args["overhead"])),
        "accounting.summarize" => {
            accounting::summarize(args["records"].as_array().ok_or("expected records")?)?
        }
        "accounting.fmt_tokens" => {
            let n = match &args["n"] {
                Value::Null => None,
                Value::String(s) => Some(s.parse::<f64>().map_err(|e| e.to_string())?),
                Value::Bool(b) => Some(u8::from(*b) as f64),
                n => Some(n.as_f64().ok_or("expected token count")?),
            };
            json!(accounting::fmt_tokens(n))
        }
        "names.repo_filename" => json!(names::repo_filename(
            args["repo"].as_str().ok_or("expected repository label")?
        )),
        op if op.starts_with("lifecycle.") => {
            let mut life: Lifecycle =
                serde_json::from_value(args["self"].clone()).map_err(|e| e.to_string())?;
            match op {
                "lifecycle._drifted_confirms" => json!(life.drifted_confirms()),
                "lifecycle.score" => {
                    json!(life.score(args["now"].as_i64().ok_or("expected task index")?))
                }
                "lifecycle.firing_rate" => json!(life.firing_rate()),
                "lifecycle.injectable" => json!(life.injectable()),
                "lifecycle.update" => {
                    life.update(
                        args["now"].as_i64().ok_or("expected task index")?,
                        args["verdict"].as_str().unwrap_or(""),
                        args["fired"].as_bool().unwrap_or(false),
                        args["exercised"].as_bool().unwrap_or(false),
                        serde_json::from_value(args["files"].clone()).map_err(|e| e.to_string())?,
                    );
                    json!(life)
                }
                _ => return Err(format!("unknown operation {op}")),
            }
        }
        op => return Err(format!("unknown operation {op}")),
    };
    Ok(result)
}

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    for line in stdin.lock().lines() {
        let response = match serde_json::from_str::<Value>(&line?) {
            Ok(request) => match dispatch(&request) {
                Ok(value) => json!({"result": value}),
                Err(error) => json!({"error": error}),
            },
            Err(error) => json!({"error": error.to_string()}),
        };
        serde_json::to_writer(&mut stdout, &response)?;
        stdout.write_all(b"\n")?;
        stdout.flush()?;
    }
    Ok(())
}
