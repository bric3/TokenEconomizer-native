// Converted from reference/upstream/tests/test_store_windows_replace.py.
mod support;

use serde_json::json;
use std::{
    fs,
    io::{self, ErrorKind},
    path::{Path, PathBuf},
};
use support::TempDir;
use tokeneconomizer::store::{REPLACE_BACKOFF, Store, read_document, write_json_atomic_with};

fn flaky_replace(
    calls: &mut Vec<PathBuf>,
    failures: usize,
) -> impl FnMut(&Path, &Path) -> io::Result<()> + '_ {
    move |source, target| {
        calls.push(target.into());
        if calls.len() <= failures {
            Err(ErrorKind::PermissionDenied.into())
        } else {
            fs::rename(source, target)
        }
    }
}

fn flaky_read<'a>(
    target: &'a Path,
    calls: &'a mut Vec<PathBuf>,
    failures: usize,
) -> impl FnMut(&Path) -> io::Result<Vec<u8>> + 'a {
    move |path| {
        if path == target {
            calls.push(path.into());
            if calls.len() <= failures {
                return Err(ErrorKind::PermissionDenied.into());
            }
        }
        fs::read(path)
    }
}

fn assert_no_temporary_files(root: &Path) {
    assert!(fs::read_dir(root).unwrap().all(|entry| {
        entry
            .unwrap()
            .path()
            .extension()
            .is_none_or(|ext| ext != "tmp")
    }));
}

#[test]
fn windows_retries_a_replace_blocked_by_an_open_file() {
    let root = TempDir::new("windows-replace-retry");
    let store = Store::new(root.path()).unwrap();
    let path = store.path(&["state.json"]);
    let mut calls = Vec::new();
    write_json_atomic_with(
        &path,
        &json!({"new":true}),
        true,
        true,
        flaky_replace(&mut calls, 2),
    )
    .unwrap();
    assert_eq!(calls.len(), 3);
    assert_eq!(
        store.read("state.json", json!(null)).unwrap(),
        json!({"new":true})
    );
    assert_no_temporary_files(root.path());
}

#[test]
fn windows_gives_up_after_a_bounded_number_of_tries() {
    let root = TempDir::new("windows-replace-bounded");
    let store = Store::new(root.path()).unwrap();
    store.write("state.json", &json!({"old":true})).unwrap();
    let mut calls = Vec::new();
    let error = write_json_atomic_with(
        &store.path(&["state.json"]),
        &json!({"new":true}),
        true,
        true,
        flaky_replace(&mut calls, 1_000_000),
    )
    .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::PermissionDenied);
    assert_eq!(calls.len(), REPLACE_BACKOFF.len() + 1);
    assert_eq!(
        store.read("state.json", json!(null)).unwrap(),
        json!({"old":true})
    );
    assert_no_temporary_files(root.path());
}

#[test]
fn posix_does_not_retry() {
    let root = TempDir::new("posix-replace-no-retry");
    let mut calls = Vec::new();
    let error = write_json_atomic_with(
        &root.path().join("state.json"),
        &json!({"new":true}),
        true,
        false,
        flaky_replace(&mut calls, 1),
    )
    .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::PermissionDenied);
    assert_eq!(calls.len(), 1);
}

#[test]
fn windows_retries_a_read_blocked_by_another_process() {
    let root = TempDir::new("windows-read-retry");
    let store = Store::new(root.path()).unwrap();
    store.write("state.json", &json!({"old":true})).unwrap();
    let path = store.path(&["state.json"]);
    let mut calls = Vec::new();
    assert_eq!(
        read_document(&path, json!({}), true, flaky_read(&path, &mut calls, 2)).unwrap(),
        json!({"old":true})
    );
    assert_eq!(calls.len(), 3);
}

#[test]
fn a_read_that_stays_blocked_raises_rather_than_reads_as_empty() {
    let root = TempDir::new("windows-read-bounded");
    let store = Store::new(root.path()).unwrap();
    store.write("state.json", &json!({"old":true})).unwrap();
    let path = store.path(&["state.json"]);
    let mut calls = Vec::new();
    let error = read_document(
        &path,
        json!({}),
        true,
        flaky_read(&path, &mut calls, 1_000_000),
    )
    .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::PermissionDenied);
    assert_eq!(calls.len(), REPLACE_BACKOFF.len() + 1);
}

#[test]
fn posix_does_not_retry_a_read() {
    let root = TempDir::new("posix-read-no-retry");
    let store = Store::new(root.path()).unwrap();
    store.write("state.json", &json!({"old":true})).unwrap();
    let path = store.path(&["state.json"]);
    let mut calls = Vec::new();
    let error =
        read_document(&path, json!({}), false, flaky_read(&path, &mut calls, 1)).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::PermissionDenied);
    assert_eq!(calls.len(), 1);
}

#[test]
fn a_missing_or_broken_document_reads_as_the_default() {
    for content in [
        None,
        Some(b"{not json".as_slice()),
        Some(b"\xff\xfe".as_slice()),
    ] {
        let root = TempDir::new("store-broken-document");
        let store = Store::new(root.path()).unwrap();
        if let Some(content) = content {
            fs::write(store.path(&["state.json"]), content).unwrap();
        }
        assert_eq!(
            store.read("state.json", json!({"default":true})).unwrap(),
            json!({"default":true})
        );
    }
}

#[cfg(windows)]
#[test]
fn a_write_lands_once_a_reader_lets_go_of_the_file() {
    use std::{os::windows::fs::OpenOptionsExt, thread, time::Duration};
    let root = TempDir::new("windows-held-reader");
    let store = Store::new(root.path()).unwrap();
    store.write("state.json", &json!({"old":true})).unwrap();
    // Match Python's reader sharing: permit reads and writes while refusing deletion.
    let held = fs::OpenOptions::new()
        .read(true)
        .share_mode(0x1 | 0x2)
        .open(store.path(&["state.json"]))
        .unwrap();
    let release = thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        drop(held);
    });
    let result = store.write("state.json", &json!({"new":true}));
    release.join().unwrap();
    result.unwrap();
    assert_eq!(
        store.read("state.json", json!(null)).unwrap(),
        json!({"new":true})
    );
    assert_no_temporary_files(root.path());
}
