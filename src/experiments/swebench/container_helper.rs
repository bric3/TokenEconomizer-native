use super::{Failure, Result, Runtime};
use crate::value::scalar_text;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) fn quote(text: &str) -> String {
    if text.is_empty() {
        "''".into()
    } else if text
        .bytes()
        .all(|c| c.is_ascii_alphanumeric() || b"_@%+=:,./-".contains(&c))
    {
        text.into()
    } else {
        format!("'{}'", text.replace('\'', "'\"'\"'"))
    }
}
fn error(message: impl Into<String>) -> Failure {
    Failure::new("RuntimeError", message)
}
pub(super) fn root(path: Option<&str>) -> Result<PathBuf> {
    Ok(match path {
        Some(path) => PathBuf::from(path),
        None => std::env::current_exe()?.with_file_name("container-helpers"),
    })
}
pub(in crate::experiments) fn artifact(root: &Path, architecture: &str) -> Result<PathBuf> {
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

pub(super) fn copy(root: &Path, home: &str, name: &Value, rt: &mut dyn Runtime) -> Result<String> {
    let probe = rt.call("_exec", json!([name, "uname -m", {"timeout":60}]))?;
    if probe["returncode"] != 0 {
        return Err(error("could not determine the container architecture"));
    }
    let architecture = probe["stdout"]
        .as_str()
        .ok_or_else(|| error("container architecture must be text"))?
        .trim();
    let artifact = artifact(root, architecture)?;
    let ready = rt.call(
        "_exec",
        json!([name, format!("mkdir -p {} && echo ready", quote(home)), {"timeout":60}]),
    )?;
    if ready["returncode"] != 0
        || !ready["stdout"]
            .as_str()
            .unwrap_or("")
            .lines()
            .any(|line| line == "ready")
    {
        return Err(error("could not create the container helper directory"));
    }
    let path = format!("{home}/te-net-guard");
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
        let message = copied["stderr"].as_str().unwrap_or("");
        return Err(error(
            message
                .chars()
                .skip(message.chars().count().saturating_sub(200))
                .collect::<String>(),
        ));
    }
    Ok(path)
}
