use serde_json::json;
use std::{fs, process};
use tokeneconomizer::trace::jsonl::{lines, records};

#[test]
fn records_skip_invalid_rows_and_replace_invalid_utf8() {
    let path = std::env::temp_dir().join(format!("te-jsonl-invalid-{}.jsonl", process::id()));
    fs::write(
        &path,
        b" \r\n\n\x1c{}\x1f\r[1]\nnull\r\nbroken\n{\"n\":17}\r{\"text\":\"bad \xff\"}\n{\"half\":",
    )
    .unwrap();
    let mut input = records(&path);
    assert_eq!(input.next(), Some(json!({})));
    assert_eq!(input.next(), Some(json!({"n":17})));
    assert_eq!(input.next(), Some(json!({"text":"bad \u{fffd}"})));
    assert_eq!(input.next(), None);
    assert_eq!(input.next(), None);
    drop(input);
    fs::remove_file(path).unwrap();
}

#[test]
fn line_endings_and_unicode_survive_read_buffer_boundaries() {
    let path = std::env::temp_dir().join(format!("te-jsonl-boundaries-{}.jsonl", process::id()));
    for padding in [
        0, 1, 8178, 8179, 8180, 8181, 8189, 8190, 8191, 8192, 8193, 16383,
    ] {
        let text = format!("{}é😀", "x".repeat(8190));
        let first = json!({"text":text}).to_string();
        let last = "{\"last\":true}";
        fs::write(
            &path,
            format!("{}\r\n {first} \r\n[]\r \n{last}", " ".repeat(padding)),
        )
        .unwrap();
        assert_eq!(
            lines(&path).collect::<Vec<_>>(),
            [first, "[]".into(), last.into()]
        );
        assert_eq!(
            records(&path).collect::<Vec<_>>(),
            [json!({"text":text}), json!({"last":true})]
        );
    }
    fs::remove_file(path).unwrap();
}
