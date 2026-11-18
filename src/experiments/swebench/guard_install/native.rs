use super::{exec, field, output, quote, space, tail};
use crate::experiments::swebench::{Failure, Result, Runtime};
use crate::value::{json_text, scalar_text, truthy};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn error(message: impl Into<String>) -> Failure {
    Failure::new("RuntimeError", message)
}

fn helper(root: &Path, architecture: &str) -> Result<PathBuf> {
    let (architecture, machine) = match architecture {
        "x86_64" | "amd64" => ("amd64", 62u16),
        "aarch64" | "arm64" => ("arm64", 183u16),
        other => {
            return Err(error(format!(
                "unsupported container architecture: {other}"
            )));
        }
    };
    let directory = root.join(architecture);
    let path = directory.join("te-net-guard");
    let bytes = fs::read(&path).map_err(|e| Failure::for_path(e, &path))?;
    let manifest_path = directory.join("manifest.json");
    let manifest: Value = serde_json::from_slice(
        &fs::read(&manifest_path).map_err(|e| Failure::for_path(e, &manifest_path))?,
    )
    .map_err(|e| error(format!("invalid container helper manifest: {e}")))?;
    if manifest["platform"] != format!("linux/{architecture}")
        || manifest["binary"] != "te-net-guard"
        || manifest["sha256"] != format!("{:x}", Sha256::digest(&bytes))
    {
        return Err(error("container helper manifest does not match its binary"));
    }
    if bytes.len() < 64
        || &bytes[..6] != b"\x7fELF\x02\x01"
        || u16::from_le_bytes(bytes[18..20].try_into().unwrap()) != machine
    {
        return Err(error(
            "container helper is not an ELF64 binary for the container architecture",
        ));
    }
    let offset = u64::from_le_bytes(bytes[32..40].try_into().unwrap());
    let size = u16::from_le_bytes(bytes[54..56].try_into().unwrap()) as u64;
    let count = u16::from_le_bytes(bytes[56..58].try_into().unwrap()) as u64;
    if size < 56 || count == 0 {
        return Err(error("invalid container helper ELF program headers"));
    }
    for index in 0..count {
        let end = offset
            .checked_add(index * size)
            .and_then(|at| at.checked_add(56))
            .and_then(|at| usize::try_from(at).ok())
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| error("invalid container helper ELF program headers"))?;
        if u32::from_le_bytes(bytes[end - 56..end - 52].try_into().unwrap()) == 3 {
            return Err(error("container helper requires a dynamic interpreter"));
        }
    }
    fs::canonicalize(&path).map_err(|e| Failure::for_path(e, &path))
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
        let path = format!("{home}/te-net-guard");
        let settings = match agent {
            "claude" => format!("{home}/.claude/settings.json"),
            "copilot" => format!("{home}/.copilot/settings.json"),
            "codex" => format!(
                "{}/hooks.json",
                scalar_text(&rt.call("codex_constant", json!(["C_HOME"]))?)
            ),
            _ => return Err(error(format!("unknown guard agent: {agent}"))),
        };
        let probe = exec(rt, name, "uname -m".into())?;
        if probe["returncode"] != 0 {
            return Err(error("could not determine the container architecture"));
        }
        let root = if let Some(root) = args["helper_root"].as_str() {
            PathBuf::from(root)
        } else {
            std::env::current_exe()?.with_file_name("container-helpers")
        };
        let artifact = helper(
            &root,
            output(&probe["stdout"], "strip")?.trim_matches(space),
        )?;
        checked(
            &exec(rt, name, format!("mkdir -p {} && echo ready", quote(home)))?,
            "ready",
        )?;
        let copied = rt.call(
            "container_run",
            json!([
                [
                    "docker",
                    "cp",
                    artifact,
                    format!("{}:{path}", scalar_text(name))
                ],
                120
            ]),
        )?;
        if copied["returncode"] != 0 {
            return Err(error(tail(output(&copied["stderr"], "strip")?, 200)));
        }
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
