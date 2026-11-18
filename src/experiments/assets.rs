use std::{fs, io, path::PathBuf};

pub(super) const FILES: &[(&str, &[u8])] = &[
    (
        "assets/codex_base_prompt/0.153.1.md",
        include_str!("assets/codex_base_prompt/0.153.1.md").as_bytes(),
    ),
    (
        "assets/webbench_undici_preload.mjs",
        include_bytes!("assets/webbench_undici_preload.mjs"),
    ),
];

pub(super) fn installed() -> io::Result<Option<PathBuf>> {
    let executable = std::env::current_exe()?;
    let Some(bin) = executable
        .parent()
        .filter(|path| path.file_name().is_some_and(|name| name == "bin"))
    else {
        return Ok(None);
    };
    let root = bin
        .parent()
        .ok_or_else(|| io::Error::other("missing runtime root"))?;
    let bytes = match fs::read(root.join("manifest.json")) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let manifest: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok((manifest["name"] == "tokeneconomizer-experiments").then(|| root.join("assets")))
}

pub(super) fn capture(version: &str) -> Option<&'static str> {
    let path = format!("assets/codex_base_prompt/{version}.md");
    FILES
        .iter()
        .find(|(name, _)| *name == path)
        .map(|(_, bytes)| std::str::from_utf8(bytes).expect("embedded captures are UTF-8"))
}
