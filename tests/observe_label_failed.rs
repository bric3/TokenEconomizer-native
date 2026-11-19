// Converted from reference/upstream/tests/test_observe_label_failed.py.
use tokeneconomizer::microskills::observe::label_failed;

#[test]
fn label_failed_matches_the_labels_own_module_without_deprecation_warnings() {
    assert!(label_failed(
        &["tests.app:Case".into()],
        "No module named 'tests'"
    ));
    assert!(!label_failed(
        &["tests/app.py".into()],
        "No module named 'pytest'"
    ));
}
