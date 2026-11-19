use crate::store::Store;
use std::{
    io,
    path::{Path, PathBuf},
};

pub(super) fn relative(path: &Path, base: &Path) -> io::Result<PathBuf> {
    let path = Store::new(path)?.base;
    let base = Store::new(base)?.base;
    let path: Vec<_> = path.components().collect();
    let base: Vec<_> = base.components().collect();
    let common = path.iter().zip(&base).take_while(|(a, b)| a == b).count();
    let mut result = PathBuf::new();
    for _ in common..base.len() {
        result.push("..");
    }
    for component in &path[common..] {
        result.push(component);
    }
    if result.as_os_str().is_empty() {
        result.push(".");
    }
    Ok(result)
}
