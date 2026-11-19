// Converted from reference/upstream/tests/test_windows_identity.py.
mod support;

use std::fs;
use support::TempDir;
use tokeneconomizer::{
    microskills::{distiller::Distiller, envfacts::EnvFactsDistiller, observe::Vocabulary},
    names::repo_filename,
    plugin::{paths::normalize_remote, repos::rename_label},
    toolprune::codex::CodexUntoolProfile,
};

#[test]
fn local_remote_spellings_share_identity() {
    for spelling in [
        r"C:\Work\Sample\Repo",
        r"C:\\Work\\Sample\\Repo",
        "C:/Work/Sample/Repo.git",
    ] {
        assert_eq!(
            normalize_remote(spelling),
            "c:/work/sample/repo",
            "{spelling}"
        );
    }
    assert_eq!(
        normalize_remote("git@example.com:Owner/Repo.git"),
        "example.com/owner/repo"
    );
    assert_eq!(
        normalize_remote("https://example.com/Owner/Repo"),
        "example.com/owner/repo"
    );
}

#[test]
fn state_writers_accept_windows_labels() {
    let label = r"C:\Work\Sample\Repo";
    let root = TempDir::new("windows-label-writers");
    Distiller::new(label, "agent", false)
        .save(root.path())
        .unwrap();
    assert!(Distiller::path(root.path(), label).unwrap().is_file());
    EnvFactsDistiller::new(label, Vocabulary::default(), None, None)
        .save(root.path())
        .unwrap();
    assert!(
        EnvFactsDistiller::path(root.path(), label)
            .unwrap()
            .is_file()
    );
    let profile = CodexUntoolProfile::new(label, root.path(), true).unwrap();
    assert!(
        !profile
            .path
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .contains(':')
    );
    rename_label(root.path(), label).unwrap();
}

#[test]
fn repo_labels_are_valid_single_filenames() {
    for label in ["owner/repo", r"C:\Work\Repo", "owner:repo", "name?bad", ""] {
        let filename = format!("{}.json", repo_filename(label));
        assert!(
            !filename.chars().any(|ch| "<>:\"/\\|?*".contains(ch)),
            "{label}: {filename}"
        );
        let root = TempDir::new("windows-label-file");
        let path = root.path().join(filename);
        fs::write(&path, "{}").unwrap();
        assert!(path.is_file());
    }
    assert_eq!(repo_filename("owner/repo"), "owner__repo");
    assert_eq!(repo_filename(".dotfiles"), ".dotfiles");
}
