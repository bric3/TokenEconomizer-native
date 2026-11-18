use crate::{
    package::{self, Result},
    plugin::paths,
};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions, Permissions},
    path::{Path, PathBuf},
};
use zip::ZipWriter;

struct Payload {
    name: String,
    bytes: Vec<u8>,
    permissions: Option<Permissions>,
}
impl Payload {
    fn file(name: String, source: &Path) -> Result<Self> {
        Ok(Self {
            name,
            bytes: fs::read(source)?,
            permissions: Some(fs::metadata(source)?.permissions()),
        })
    }
    fn embedded(name: &str, bytes: &[u8]) -> Self {
        Self {
            name: name.into(),
            bytes: bytes.into(),
            permissions: None,
        }
    }
}

/// Build a fresh runtime payload without reading the upstream checkout.
pub fn build(binaries: &Path, helpers: &Path, out: &Path) -> Result<PathBuf> {
    package::validate_notices()?;
    let binaries = fs::canonicalize(binaries)?;
    let helpers = fs::canonicalize(helpers)?;
    let out = paths::realpath(out)?;
    for input in [&binaries, &helpers] {
        if out.starts_with(input) || input.starts_with(&out) {
            return Err("experiment output overlaps its inputs".into());
        }
    }
    if out.exists() {
        return Err("experiment output already exists; choose a fresh output directory".into());
    }
    let suffix = if binaries.join("te.exe").is_file() {
        ".exe"
    } else {
        ""
    };
    let mut files = Vec::new();
    for name in [
        "te",
        "te-hook",
        "te-bridge",
        "te-net-guard",
        "te-repomap-gen",
    ] {
        let name = format!("{name}{suffix}");
        let source = binaries.join(&name);
        if !source.is_file() || !paths::executable(&source)? {
            return Err(format!("missing executable: {}", source.display()).into());
        }
        files.push(Payload::file(format!("bin/{name}"), &source)?);
    }
    let lock_hash = format!("{:x}", Sha256::digest(include_bytes!("../../Cargo.lock")));
    for (architecture, machine) in [("arm64", "aarch64"), ("amd64", "x86_64")] {
        let helper = super::swebench::container_helper::artifact(&helpers, machine)
            .map_err(|error| format!("{}: {}", error.kind, error.message))?;
        let directory = helpers.join(architecture);
        let manifest: Value = serde_json::from_slice(&fs::read(directory.join("manifest.json"))?)?;
        if manifest["cargo_lock_sha256"] != lock_hash
            || fs::read(directory.join("THIRD-PARTY-NOTICES.txt"))? != package::NOTICES.as_bytes()
        {
            return Err(
                "container helper dependencies or notices differ from this packager".into(),
            );
        }
        let binary = Payload::file(
            format!("bin/container-helpers/{architecture}/te-net-guard"),
            &helper,
        )?;
        if manifest["sha256"] != format!("{:x}", Sha256::digest(&binary.bytes)) {
            return Err("container helper changed while building the bundle".into());
        }
        files.push(binary);
        for name in ["manifest.json", "toolchain.txt", "THIRD-PARTY-NOTICES.txt"] {
            files.push(Payload::file(
                format!("bin/container-helpers/{architecture}/{name}"),
                &directory.join(name),
            )?);
        }
    }
    for (name, bytes) in super::assets::FILES {
        files.push(Payload::embedded(name, bytes));
    }
    files.push(Payload::embedded(
        "LICENSE",
        include_bytes!("../../LICENSE"),
    ));
    files.push(Payload::embedded(
        "THIRD-PARTY-NOTICES.txt",
        package::NOTICES.as_bytes(),
    ));
    files.push(Payload::embedded(
        "README.md",
        include_bytes!("bundle/README.md"),
    ));
    let hashes: Map<String, Value> = files
        .iter()
        .map(|file| {
            (
                file.name.clone(),
                json!(format!("{:x}", Sha256::digest(&file.bytes))),
            )
        })
        .collect();
    let manifest = json!({"format":1,"name":"tokeneconomizer-experiments","version":env!("CARGO_PKG_VERSION"),"upstream_commit":"583fff78bf504b59d542f953c31e87d01d02ab42","cargo_lock_sha256":lock_hash,"entrypoint":format!("bin/te-bridge{suffix}"),"files":hashes});
    // Validate and snapshot every input before creating the output directory.
    fs::create_dir_all(out.parent().ok_or("missing experiment output parent")?)?;
    fs::create_dir(&out)?;
    for file in files {
        let destination = out.join(&file.name);
        fs::create_dir_all(destination.parent().unwrap())?;
        fs::write(&destination, file.bytes)?;
        if let Some(permissions) = file.permissions {
            fs::set_permissions(destination, permissions)?;
        }
    }
    fs::write(
        out.join("manifest.json"),
        format!("{}\n", serde_json::to_string_pretty(&manifest)?),
    )?;
    Ok(out)
}

pub fn archive(bundle: &Path) -> Result<PathBuf> {
    let mut name = bundle.as_os_str().to_owned();
    name.push(".zip");
    let path = PathBuf::from(name);
    let file: File = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    let mut zip = ZipWriter::new(file);
    package::archive_tree(
        &mut zip,
        bundle.parent().ok_or("missing bundle parent")?,
        bundle,
    )?;
    zip.finish()?;
    Ok(path)
}
