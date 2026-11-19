use std::{io, path::PathBuf};

pub(super) const FILES: &[(&str, &[u8])] = &[
    ("sdk/bench.py", include_bytes!("sdk/bench.py")),
    (
        "sdk/te_runtime/__init__.py",
        include_bytes!("sdk/te_runtime/__init__.py"),
    ),
    (
        "sdk/te_runtime/transport.py",
        include_bytes!("sdk/te_runtime/transport.py"),
    ),
    (
        "sdk/te_runtime/driver.py",
        include_bytes!("sdk/te_runtime/driver.py"),
    ),
    (
        "sdk/te_runtime/codex_spiral.py",
        include_bytes!("sdk/te_runtime/codex_spiral.py"),
    ),
    (
        "sdk/te_runtime/nudge_guard.py",
        include_bytes!("sdk/te_runtime/nudge_guard.py"),
    ),
    (
        "sdk/te_runtime/codex_install.py",
        include_bytes!("sdk/te_runtime/codex_install.py"),
    ),
];

/// Resolve the SDK bootstrap and the caller's configuration root.
pub fn script_and_root() -> io::Result<(PathBuf, PathBuf)> {
    if let Some(assets) = super::assets::installed()? {
        let root = assets
            .parent()
            .ok_or_else(|| io::Error::other("missing runtime root"))?
            .to_path_buf();
        return Ok((root.join("sdk/bench.py"), root));
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    Ok((root.join("src/experiments/sdk/bench.py"), root))
}
