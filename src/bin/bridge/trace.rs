use std::path::Path;

use serde_json::{Value, json};
use tokeneconomizer::trace::{
    antigravity, calls, catalog, codex, copilot, cursor, grok, opencode, pi,
};

fn path<'a>(args: &'a Value, key: &str) -> &'a Path {
    Path::new(args[key].as_str().unwrap_or(""))
}

fn strings(args: &Value, key: &str) -> Result<Vec<String>, String> {
    serde_json::from_value(args[key].clone()).map_err(|e| e.to_string())
}

pub fn dispatch(op: &str, args: &Value) -> Result<Option<Value>, String> {
    let value = match op {
        "trace.grok.grok_tool" => json!(grok::tool(&args["name"], &args["args"])),
        "trace.grok._bytes_text" => json!(grok::bytes_text(&args["value"])),
        "trace.grok.grok_result" => json!(grok::result(&args["resp"])),
        "trace.grok._content_text" => json!(grok::content_text(&args["content"])),
        "trace.grok.grok_session_to_claude" => json!(grok::session_to_claude(
            path(args, "path"),
            path(args, "out_path")
        )?),
        "trace.grok.grok_prompt_count" => json!(grok::prompt_count(path(args, "path"))),
        "trace.grok.grok_session_usage" => grok::session_usage(path(args, "path"))?,
        "trace.grok.grok_json_reply" => {
            json!(grok::json_reply(args["stdout"].as_str().unwrap_or(""))?)
        }
        "trace.cursor._response_text" => json!(cursor::response_text(&args["response"])),
        "trace.cursor._exit_code" => json!(cursor::exit_code(&args["response"])),
        "trace.cursor.cursor_events_to_claude" => json!(cursor::events_to_claude(
            path(args, "path"),
            path(args, "out_path")
        )),
        "trace.cursor.cursor_prompt_count" => json!(cursor::prompt_count(path(args, "path"))),
        "trace.antigravity.antigravity_tool" => {
            json!(antigravity::tool(&args["name"], &args["args"]))
        }
        "trace.antigravity.antigravity_result" => json!(antigravity::result(&args["row"])),
        "trace.antigravity._prompt_text" => json!(antigravity::prompt_text(&args["row"])),
        "trace.antigravity._read_rows" => json!(antigravity::read_rows(
            path(args, "path"),
            args["offset"].as_u64().unwrap_or(0)
        )),
        "trace.antigravity._is_result" => json!(antigravity::is_result(&args["row"])),
        "trace.antigravity.antigravity_read" => json!(antigravity::read(
            path(args, "path"),
            args["offset"].as_u64().unwrap_or(0),
            args["after"].as_i64().unwrap_or(-1),
            args["final"].as_bool().unwrap_or(true)
        )),
        "trace.antigravity.antigravity_session_to_claude" => json!(antigravity::session_to_claude(
            path(args, "path"),
            path(args, "out_path")
        )),
        "trace.antigravity.antigravity_session_usage" => {
            antigravity::session_usage(path(args, "path"))
        }
        "trace.antigravity.antigravity_prompt_count" => {
            json!(antigravity::prompt_count(path(args, "path")))
        }
        "trace.pi.pi_session_header" => pi::session_header(path(args, "path")),
        "trace.pi.pi_active_branch" => json!(pi::active_branch(
            args["entries"].as_array().ok_or("expected entries")?
        )),
        "trace.pi.pi_prompt_count" => json!(pi::prompt_count(path(args, "path"))),
        "trace.pi._pi_usage" => json!(pi::usage(&args["u"])?),
        "trace.pi._pi_text" => json!(pi::text(&args["content"])),
        "trace.pi.pi_session_to_claude" => json!(pi::session_to_claude(
            path(args, "path"),
            path(args, "out_path")
        )?),
        "trace.pi.pi_json_reply" => json!(pi::json_reply(args["stdout"].as_str().unwrap_or(""))?),
        "trace.pi.pi_session_usage" => pi::session_usage(path(args, "path"))?,
        "trace.opencode._nested" => opencode::nested(&args["msg"], args["cwd"].as_str()),
        "trace.opencode._load" => json!(opencode::load(path(args, "path"))),
        "trace.opencode._text" => json!(opencode::text(&args["msg"])),
        "trace.opencode._usage" => json!(opencode::usage(&args["info"])?),
        "trace.opencode.opencode_session_info" => opencode::session_info(path(args, "path")),
        "trace.opencode.opencode_prompt_count" => json!(opencode::prompt_count(path(args, "path"))),
        "trace.opencode.opencode_session_to_claude" => json!(opencode::session_to_claude(
            path(args, "path"),
            path(args, "out_path")
        )?),
        "trace.opencode.opencode_session_usage" => opencode::session_usage(path(args, "path"))?,
        "trace.opencode.opencode_run_reply" => {
            json!(opencode::run_reply(args["stdout"].as_str().unwrap_or(""))?)
        }
        "trace.catalog.parse_tool_catalog_flat" => json!(catalog::parse_flat(
            args["debug_log_text"].as_str().unwrap_or("")
        )),
        "trace.catalog.parse_tool_catalog" => json!(catalog::parse_schemas(
            args["debug_log_text"].as_str().unwrap_or(""),
            args["max_desc"].as_i64().unwrap_or(200)
        )),
        "trace.catalog._load_claude_catalog_snapshot" => catalog::load_claude_snapshot(
            args["path"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(Path::new),
        ),
        "trace.catalog.parse_tool_catalog_claude" => json!(catalog::parse_claude(
            args["transcript_text"].as_str().unwrap_or(""),
            args["max_desc"].as_i64().unwrap_or(200),
            args.get("snapshot").filter(|s| !s.is_null())
        )),
        "trace.catalog.parse_tool_names_copilot" => json!(catalog::parse_copilot_names(
            args["transcript_text"].as_str().unwrap_or("")
        )),
        "trace.catalog.catalog_sources" => json!(catalog::catalog_sources(
            path(args, "run_dir_path"),
            args["agent"].as_str().unwrap_or("")
        )),
        "trace.catalog.parse_catalog_with_source" => json!(catalog::parse_with_source(
            path(args, "run_dir_path"),
            args["agent"].as_str().unwrap_or("")
        )),
        "trace.catalog.parse_catalog" => json!(catalog::parse(
            path(args, "run_dir_path"),
            args["agent"].as_str().unwrap_or("")
        )),
        "trace.catalog._catalog_rank" => json!(catalog::rank(
            args["item"][1].as_object().ok_or("expected catalog")?
        )),
        "trace.calls.strip_rtk_prefix" => json!(calls::strip_rtk_prefix(
            args["command"].as_str().unwrap_or("")
        )),
        "trace.calls._is_codex_exec" => json!(calls::is_codex_exec(&strings(args, "lines")?)),
        "trace.calls.load_trajectory" => json!(calls::load_trajectory(path(args, "path"))),
        "trace.calls.call_cwds" => json!(calls::call_cwds(path(args, "path"))),
        "trace.calls.tool_usage" => json!(calls::tool_usage(path(args, "path"))),
        "trace.codex._shell_patch_paths" => json!(codex::shell_patch_paths(
            args["command"].as_str().unwrap_or("")
        )),
        "trace.codex._command_text" => json!(codex::command_text(&args["command"])),
        "trace.codex.codex_exec_calls" => json!(codex::exec_calls(&strings(args, "lines")?)),
        "trace.codex.rollout_item_to_exec" => json!(codex::rollout_item_to_exec(&args["item"])),
        "trace.codex.codex_rollout_to_exec" => json!(codex::rollout_to_exec(
            path(args, "rollout_path"),
            path(args, "out_path")
        )?),
        "trace.codex.codex_capability_trace" => json!(codex::capability_trace(
            path(args, "rollout_path"),
            path(args, "out_path")
        )?),
        "trace.codex.codex_prompt_count" => json!(codex::prompt_count(path(args, "rollout_path"))),
        "trace.codex.codex_session_meta" => codex::session_meta(path(args, "rollout_path")),
        "trace.codex.codex_session_usage" => codex::session_usage(path(args, "rollout_path"))?,
        "trace.copilot.copilot_event_calls" => {
            json!(copilot::event_calls(&strings(args, "lines")?))
        }
        "trace.copilot.copilot_session_cwd" => {
            json!(copilot::session_cwd(&strings(args, "lines")?))
        }
        "trace.copilot.copilot_session_meta" => copilot::session_meta(path(args, "path")),
        "trace.copilot.copilot_session_usage" => copilot::session_usage(path(args, "path"))?,
        "trace.copilot.copilot_turns" => json!(copilot::turns(path(args, "path"))),
        "trace.copilot.copilot_prompt_count" => json!(copilot::prompt_count(path(args, "path"))),
        "trace.copilot.copilot_mcp_servers" => {
            json!(copilot::mcp_servers(&args["env"], path(args, "home")))
        }
        "trace.copilot.mcp_server_of" => json!(copilot::mcp_server_of(
            args["tool"].as_str().ok_or("expected tool")?,
            &strings(args, "servers")?
        )),
        _ => return Ok(None),
    };
    Ok(Some(value))
}
