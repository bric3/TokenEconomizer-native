// Converted from reference/upstream/tests/test_loopguard_productive_turns.py.
use serde_json::{Value, json};
use tokeneconomizer::{
    loopguard::{
        shell_edits::{shell_edit_targets, shell_revert_targets},
        turn::{self, FoldOptions, TurnGuard},
    },
    plugin::hosts,
};

const LIBUV: &str =
    r"Assertion failed: !(handle->flags & UV_HANDLE_CLOSING), file src\win\async.c, line 76";
fn fold(guard: &mut TurnGuard, name: &str, args: &Value, text: &str, code: Option<i64>) {
    guard
        .fold(name, args, text, code, FoldOptions::default())
        .unwrap();
}

#[test]
fn an_exit_status_covers_a_command_unless_its_work_could_fail_unseen() {
    for (cmd, expected) in [
        ("npx playwright test spec.ts", true),
        ("cd apps/web && npx vitest run a.test.ts", true),
        ("cd apps/web; npx vitest run a.test.ts", true),
        ("cat apps/web/booking.ts", true),
        ("grep -rn BookingRequestError apps | head -20", true),
        ("git show HEAD:apps/web/booking.ts | sed -n '1,80p'", true),
        ("set -o pipefail; python repro.py | tail -5", true),
        ("python repro.py 2>&1 | tail -20", false),
        ("npm test; echo \"exit=$?\"", false),
        ("sed -i 's/a/b/' x.ts; npm test", false),
        ("make || true", false),
        (
            "SP=/tmp/x; cd $SP && python3 - <<'EOF'\nimport json\nprint('error outputs: 3')\nEOF",
            true,
        ),
        (
            "python3 - <<'EOF'\nprint(1)\nEOF\npython -m pytest -q | tail -3",
            false,
        ),
        ("python repro.py <<< y", true),
        (
            "python repro.py <<< \"y\"\npython -m pytest -q | tail -3",
            false,
        ),
        ("", false),
    ] {
        assert_eq!(turn::exit_status_covers(cmd).unwrap(), expected, "{cmd}");
    }
}

#[test]
fn a_claude_code_shell_success_exited_zero_where_that_status_covers_it() {
    for (cmd, text, interrupted, code) in [
        ("npx playwright test spec.ts", LIBUV, false, json!(0)),
        (
            "python repro.py 2>&1 | tail",
            "Traceback",
            false,
            Value::Null,
        ),
        ("npm test", "", true, Value::Null),
    ] {
        assert_eq!(hosts::tool_call("claude", &json!({"hook_event_name":"PostToolUse","tool_name":"Bash","tool_input":{"command":cmd},"tool_response":{"stdout":text,"stderr":"","interrupted":interrupted}})).unwrap().3, code);
    }
    assert!(hosts::tool_call("claude", &json!({"hook_event_name":"PostToolUse","tool_name":"Read","tool_input":{"file_path":"/r/a.ts"},"tool_response":"x"})).unwrap().3.is_null());
}

#[test]
fn green_playwright_runs_are_not_failures_whatever_libuv_prints() {
    let mut guard = TurnGuard::default();
    for n in [7, 5] {
        let output = format!("{LIBUV}\n  {n} passed (23.6s)");
        fold(
            &mut guard,
            "Edit",
            &json!({"file_path":"/r/apps/web/a.ts"}),
            "ok",
            None,
        );
        fold(
            &mut guard,
            "Bash",
            &json!({"command":"npx playwright test spec.ts"}),
            &output,
            Some(0),
        );
        fold(
            &mut guard,
            "Bash",
            &json!({"command":"npx playwright test spec.ts 2>&1 | tail -5"}),
            &output,
            None,
        );
    }
    assert_eq!(guard.whole.error_outputs, 0);
    assert_eq!(guard.whole.repeated_fail_sigs, 0);
    assert!(guard.resolved_proxy(true));
    assert_eq!(guard.whole.test_runs, 4);
    assert_eq!(
        turn::runner_verdict("  1 failed\n    [chromium] a.spec.ts:3:1\n  6 passed (20.1s)"),
        Some(true)
    );
}

#[test]
fn source_read_through_the_shell_is_not_an_error_output_and_a_piped_crash_still_is() {
    let mut guard = TurnGuard::default();
    let source =
        r#"      if (!service) throw new BookingRequestError("unavailable", "Service not found");"#;
    for cmd in [
        "cat apps/web/booking.ts",
        "sed -n '1,80p' apps/web/booking.ts",
        "grep -rn BookingRequestError apps | head -20",
    ] {
        let args = json!({"command":cmd});
        fold(
            &mut guard,
            "Bash",
            &args,
            source,
            hosts::claude_success_code("Bash", &args, None).unwrap(),
        );
    }
    assert_eq!(guard.whole.error_outputs, 0);
    let args = json!({"command":"python repro.py 2>&1 | tail -3"});
    fold(
        &mut guard,
        "Bash",
        &args,
        "Traceback (most recent call last):\nValueError: x",
        hosts::claude_success_code("Bash", &args, None).unwrap(),
    );
    assert_eq!(guard.whole.error_outputs, 1);
}

#[test]
fn putting_back_a_file_the_turn_never_edited_is_not_an_edit() {
    let revert = "git checkout -q -- apps/web/AGENTS.md";
    assert_eq!(shell_edit_targets(revert).unwrap(), ["AGENTS.md"]);
    assert_eq!(shell_revert_targets(revert).unwrap(), ["AGENTS.md"]);
    let mut guard = TurnGuard::default();
    for _ in 0..4 {
        fold(&mut guard, "Bash", &json!({"command":revert}), "", Some(0));
    }
    assert_eq!(guard.whole.edit_churn(), 0);
    assert_eq!(guard.edits, 0);
    fold(
        &mut guard,
        "Edit",
        &json!({"file_path":"/r/apps/web/page.tsx"}),
        "ok",
        None,
    );
    fold(
        &mut guard,
        "Bash",
        &json!({"command":"git restore apps/web/page.tsx"}),
        "",
        Some(0),
    );
    assert_eq!(guard.whole.edit_counts["page.tsx"], 2);
    assert_eq!(guard.edits, 2);
}

#[test]
fn a_vitest_run_behind_dotenv_is_a_verification_and_a_test_run() {
    let command = "dotenv -e .env.local -- npx vitest run a.test.ts";
    assert_eq!(turn::status_of_tests(command).unwrap(), Some("own"));
    let mut guard = TurnGuard::default();
    fold(
        &mut guard,
        "Bash",
        &json!({"command":command}),
        " Tests  3 passed (3)",
        Some(0),
    );
    fold(
        &mut guard,
        "Bash",
        &json!({"command":"npx vitest run b.test.ts"}),
        " Tests  2 passed (2)",
        Some(0),
    );
    assert_eq!(guard.verifications, 2);
    assert_eq!(guard.st.test_runs, 2);
}

#[test]
fn successful_claude_edits_of_files_containing_assert_or_failed_are_not_errors() {
    let edit = json!({"hook_event_name":"PostToolUse","tool_name":"Edit","tool_input":{"file_path":"/r/README.md"},"tool_response":{"filePath":"/r/README.md","oldString":"old","newString":"the failed case asserts the value","originalFile":"old\n","structuredPatch":[{"lines":["+the failed case asserts the value"]}],"replaceAll":false,"userModified":false}});
    let write = json!({"hook_event_name":"PostToolUse","tool_name":"Write","tool_input":{"file_path":"/r/test_a.py"},"tool_response":{"type":"create","filePath":"/r/test_a.py","content":"def test_x():\n    assert False, 'failed'\n","originalFile":null,"structuredPatch":[]}});
    let mut guard = TurnGuard::default();
    for payload in [&edit, &write, &edit] {
        let (name, args, text, code) = hosts::tool_call("claude", payload).unwrap();
        assert_eq!(text, "");
        fold(
            &mut guard,
            name.as_str().unwrap(),
            &args,
            &text,
            code.as_i64(),
        );
    }
    assert_eq!(guard.whole.error_outputs, 0);
    assert_eq!(guard.whole.repeated_fail_sigs, 0);
    let (name,args,text,code) = hosts::tool_call("claude",&json!({"hook_event_name":"PostToolUseFailure","tool_name":"Edit","tool_input":{"file_path":"/r/README.md"},"error":"Error: String to replace not found in file."})).unwrap();
    fold(
        &mut guard,
        name.as_str().unwrap(),
        &args,
        &text,
        code.as_i64(),
    );
    assert_eq!(guard.whole.error_outputs, 1);
}
