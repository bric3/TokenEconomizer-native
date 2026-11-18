use super::{exec, field, output, quote, tail};
use crate::experiments::swebench::container_helper;
use crate::experiments::swebench::{Failure, Result, Runtime};
use crate::value::{json_text, scalar_text, truthy};
use serde_json::{Value, json};

fn error(message: impl Into<String>) -> Failure {
    Failure::new("RuntimeError", message)
}

fn checked(result: &Value, marker: &str) -> Result<()> {
    let stdout = output(&result["stdout"], "strip")?;
    if result["returncode"] != 0 || !stdout.lines().any(|line| line == marker) {
        let stderr = output(&result["stderr"], "strip")?;
        return Err(error(tail(&format!("{stderr}{stdout}"), 200)));
    }
    Ok(())
}

pub(super) fn install(args: &Value, rt: &mut dyn Runtime) -> Result<Value> {
    if !truthy(&rt.call("config_function", json!(["network_tool_block", []]))?) {
        return Ok(json!(false));
    }
    let result = (|| -> Result<()> {
        let name = &args["name"];
        let agent = field(rt, "scaffold_profile", "kind", "json", None)?;
        let agent = agent.as_str().ok_or_else(|| error("missing guard agent"))?;
        let home = field(rt, "scaffold_profile", "home", "json", None)?;
        let home = home
            .as_str()
            .ok_or_else(|| error("agent home must be a path string"))?;
        let settings = match agent {
            "claude" => format!("{home}/.claude/settings.json"),
            "copilot" => format!("{home}/.copilot/settings.json"),
            "codex" => format!(
                "{}/hooks.json",
                scalar_text(&rt.call("codex_constant", json!(["C_HOME"]))?)
            ),
            _ => return Err(error(format!("unknown guard agent: {agent}"))),
        };
        let root = container_helper::root(args["helper_root"].as_str())?;
        let path = container_helper::copy(&root, home, name, rt)?;
        let quoted = quote(&path);
        let registered = exec(
            rt,
            name,
            format!(
                "chmod 755 {quoted} && {quoted} --install-hook {} {} {}",
                quote(agent),
                quote(&settings),
                quote(&quoted)
            ),
        )?;
        checked(&registered, "armed")?;
        if agent == "codex" {
            // The compatibility field tells the segment to enable Codex hook trust.
            rt.call("config_set", json!(["_guard_python", path]))?;
        }
        let probe = field(rt, "runner", "NET_GUARD_PROBE", "json", None)?;
        let payload = json_text(
            &json!({"tool_name":"Bash", "toolName":"bash", "tool_input":{"command":probe}, "toolArgs":{"command":probe}}),
        );
        let refused = exec(
            rt,
            name,
            format!("printf '%s' {} | {quoted}; echo rc=$?", quote(&payload)),
        )?;
        if refused["returncode"] != 0
            || !output(&refused["stdout"], "strip")?
                .lines()
                .any(|line| line == "rc=2")
        {
            return Err(error(
                "native guard installed but did NOT refuse its probe (expected rc=2)",
            ));
        }
        Ok(())
    })();
    match result {
        Ok(()) => Ok(json!(true)),
        Err(failure) if failure.is_a("builtins.Exception") => {
            rt.call(
                "log",
                json!([format!(
                    "net guard install FAILED ({}: {}) -- row runs UNGUARDED",
                    failure.kind, failure.message
                )]),
            )?;
            Ok(json!(false))
        }
        Err(failure) => Err(failure),
    }
}
