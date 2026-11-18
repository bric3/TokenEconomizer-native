use crate::python_json::{self, Node, ParseError};
use indexmap::IndexMap;
use std::{fs, io, path::Path};

fn io_error(error: io::Error, path: &Path) -> String {
    let kind = match error.kind() {
        io::ErrorKind::NotFound => "FileNotFoundError",
        io::ErrorKind::PermissionDenied => "PermissionError",
        io::ErrorKind::AlreadyExists => "FileExistsError",
        io::ErrorKind::NotADirectory => "NotADirectoryError",
        io::ErrorKind::IsADirectory => "IsADirectoryError",
        _ => "OSError",
    };
    let message = error.to_string();
    if let Some(code) = error.raw_os_error() {
        let description = message
            .rsplit_once(" (os error ")
            .map_or(message.as_str(), |(text, _)| text);
        let path = crate::value::python_string_repr(path.to_string_lossy().chars().map(u32::from));
        format!("{kind}: [Errno {code}] {description}: {path}")
    } else {
        format!("{kind}: {message}")
    }
}

/// Merge the scaffold patch one dictionary level deep, preserving unrelated keys.
pub fn merge_json_file(
    path: &Path,
    patch_json: &[u8],
    int_max_digits: usize,
) -> Result<(), String> {
    let document =
        std::str::from_utf8(patch_json).map_err(|error| format!("UnicodeDecodeError: {error}"))?;
    let mut patch = python_json::parse_result(patch_json, int_max_digits).map_err(|error| {
        let kind = if matches!(error, ParseError::Syntax { .. }) {
            "JSONDecodeError"
        } else {
            "ValueError"
        };
        format!("{kind}: {}", error.message(document))
    })?;
    let parent = path.parent().unwrap_or(Path::new(""));
    if parent.as_os_str().is_empty() {
        return Err("FileNotFoundError: [Errno 2] No such file or directory: ''".into());
    }
    fs::create_dir_all(parent).map_err(|error| io_error(error, parent))?;
    let mut current = fs::read(path)
        .ok()
        .filter(|bytes| std::str::from_utf8(bytes).is_ok())
        .and_then(|bytes| python_json::parse(&bytes, int_max_digits))
        .unwrap_or_else(|| Node::Dict(IndexMap::new()));
    let patch_kind = patch.kind();
    let Node::Dict(patch) = &mut patch else {
        return Err(format!(
            "AttributeError: '{patch_kind}' object has no attribute 'items'"
        ));
    };
    for (key, mut value) in std::mem::take(patch) {
        let current_kind = current.kind();
        let Node::Dict(current) = &mut current else {
            return Err(if matches!(value, Node::Dict(_)) {
                format!("AttributeError: '{current_kind}' object has no attribute 'get'")
            } else if current_kind == "list" {
                "TypeError: list indices must be integers or slices, not str".into()
            } else {
                format!("TypeError: '{current_kind}' object does not support item assignment")
            });
        };
        match (current.get_mut(&key), &mut value) {
            (Some(Node::Dict(existing)), Node::Dict(fields)) => {
                existing.extend(std::mem::take(fields))
            }
            _ => {
                current.insert(key, value);
            }
        }
    }
    // Finish validation before opening the destination, as the generated source program does.
    fs::write(path, current.json(true)).map_err(|error| io_error(error, path))
}
