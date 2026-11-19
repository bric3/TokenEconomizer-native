use pcre2::bytes::{Regex, RegexBuilder};
use pcre2_sys as ffi;
use std::{collections::BTreeMap, ffi::c_void, ops::Range, ptr::NonNull};

use crate::unicode17::LOWER;

#[derive(Clone, Copy)]
pub(super) struct Reference {
    pub group: usize,
    pub ascii: bool,
    pub width: Option<usize>,
}

pub(super) enum Matcher {
    Plain(Regex),
    Checked { ascii: Regex, unicode: Checked },
}
impl Matcher {
    pub fn new(pattern: &str, references: Vec<(Range<usize>, Reference)>) -> Option<Self> {
        let ascii = RegexBuilder::new()
            .utf(true)
            .ucp(true)
            .build(pattern)
            .ok()?;
        if references.is_empty() {
            return Some(Self::Plain(ascii));
        }
        let mut translated = String::new();
        let mut positions = BTreeMap::new();
        let mut previous = 0;
        for (span, reference) in references {
            translated.push_str(&pattern[previous..span.start]);
            translated.push_str("(?>(?C1)");
            positions.insert(translated.len(), reference);
            translated.push_str(&format!(r"(?:\g{{{}}}|", reference.group));
            match reference.width {
                Some(width) => translated.push_str(&format!("(?s:.{{{width}}})")),
                None => translated.push_str("(?s:.*?)"),
            }
            translated.push_str(")(?C2))");
            previous = span.end;
        }
        translated.push_str(&pattern[previous..]);
        Checked::new(&translated, positions).map(|unicode| Self::Checked { ascii, unicode })
    }
    pub fn is_match(&self, text: &str) -> Result<bool, String> {
        match self {
            Self::Plain(regex) => regex
                .is_match(text.as_bytes())
                .map_err(|error| error.to_string()),
            Self::Checked { ascii: regex, .. } if text.is_ascii() => regex
                .is_match(text.as_bytes())
                .map_err(|error| error.to_string()),
            Self::Checked { unicode, .. } => unicode.is_match(text),
        }
    }
}

pub(super) struct Checked {
    code: NonNull<ffi::pcre2_code_8>,
    references: BTreeMap<usize, Reference>,
}
// Compiled code is immutable; each match owns its PCRE2 context, offsets and callback state.
unsafe impl Send for Checked {}
unsafe impl Sync for Checked {}

impl Checked {
    fn new(pattern: &str, references: BTreeMap<usize, Reference>) -> Option<Self> {
        let mut error = 0;
        let mut position = 0;
        // Preserve callout validation across PCRE2's anchoring and backtracking optimisations.
        let options = ffi::PCRE2_UTF
            | ffi::PCRE2_UCP
            | ffi::PCRE2_NO_DOTSTAR_ANCHOR
            | ffi::PCRE2_NO_AUTO_POSSESS;
        let code = unsafe {
            ffi::pcre2_compile_8(
                pattern.as_ptr(),
                pattern.len(),
                options,
                &mut error,
                &mut position,
                std::ptr::null_mut(),
            )
        };
        NonNull::new(code).map(|code| Self { code, references })
    }
    fn is_match(&self, subject: &str) -> Result<bool, String> {
        let mut state = State {
            subject,
            references: &self.references,
            end: 0,
            continuations: subject
                .bytes()
                .enumerate()
                .filter_map(|(i, b)| (b & 0xc0 == 0x80).then_some(i))
                .collect(),
        };
        // PCRE2 calls synchronously; state and subject remain borrowed until matching returns.
        let result = unsafe {
            let data = ffi::pcre2_match_data_create_from_pattern_8(
                self.code.as_ptr(),
                std::ptr::null_mut(),
            );
            if data.is_null() {
                return Err("PCRE2: unable to allocate match data".into());
            }
            let context = ffi::pcre2_match_context_create_8(std::ptr::null_mut());
            if context.is_null() {
                ffi::pcre2_match_data_free_8(data);
                return Err("PCRE2: unable to allocate match context".into());
            }
            let configured = pcre2_set_callout_8(
                context,
                Some(callout),
                (&mut state as *mut State<'_>).cast(),
            );
            let result = if configured == 0 {
                ffi::pcre2_match_8(
                    self.code.as_ptr(),
                    subject.as_ptr(),
                    subject.len(),
                    0,
                    0,
                    data,
                    context,
                )
            } else {
                configured
            };
            ffi::pcre2_match_context_free_8(context);
            ffi::pcre2_match_data_free_8(data);
            result
        };
        if result >= 0 {
            return Ok(true);
        }
        if result == ffi::PCRE2_ERROR_NOMATCH {
            return Ok(false);
        }
        let mut message = [0; 256];
        let length =
            unsafe { ffi::pcre2_get_error_message_8(result, message.as_mut_ptr(), message.len()) };
        if length < 0 {
            return Err(format!("PCRE2: error matching: {result}"));
        }
        Err(format!(
            "PCRE2: error matching: {}",
            String::from_utf8_lossy(&message[..length as usize])
        ))
    }
}
impl Drop for Checked {
    fn drop(&mut self) {
        unsafe { ffi::pcre2_code_free_8(self.code.as_ptr()) };
    }
}

// The version-0 prefix of pcre2_callout_block in the vendored PCRE2 header is ABI-stable.
#[repr(C)]
struct Callout {
    _version: u32,
    number: u32,
    capture_top: u32,
    _capture_last: u32,
    offsets: *const usize,
    _mark: *const u8,
    _subject: *const u8,
    _subject_length: usize,
    _start_match: usize,
    current: usize,
    pattern: usize,
}
unsafe extern "C" {
    fn pcre2_set_callout_8(
        context: *mut ffi::pcre2_match_context_8,
        callback: Option<unsafe extern "C" fn(*mut Callout, *mut c_void) -> i32>,
        data: *mut c_void,
    ) -> i32;
}
struct State<'a> {
    subject: &'a str,
    references: &'a BTreeMap<usize, Reference>,
    end: usize,
    continuations: Vec<usize>,
}
impl State<'_> {
    fn character_position(&self, byte: usize) -> usize {
        byte - self
            .continuations
            .partition_point(|&position| position < byte)
    }
}
fn lower(character: char, ascii: bool) -> char {
    if ascii || character.is_ascii() {
        return character.to_ascii_lowercase();
    }
    LOWER
        .binary_search_by_key(&character, |&(from, _)| from)
        .map(|index| LOWER[index].1)
        .unwrap_or(character)
}
unsafe extern "C" fn callout(block: *mut Callout, data: *mut c_void) -> i32 {
    // Both pointers come from this match's registered PCRE2 callback and live stack state.
    let (block, state) = unsafe { (&*block, &mut *data.cast::<State<'_>>()) };
    if block.number == 2 {
        return i32::from(block.current != state.end);
    }
    let Some(reference) = state.references.get(&block.pattern) else {
        return 1;
    };
    if reference.group >= block.capture_top as usize {
        return 1;
    }
    // capture_top bounds the readable ovector, including unset optional captures.
    let (start, end) = unsafe {
        (
            *block.offsets.add(reference.group * 2),
            *block.offsets.add(reference.group * 2 + 1),
        )
    };
    let Some(captured) = state.subject.get(start..end) else {
        return 1;
    };
    let Some(remaining) = state.subject.get(block.current..) else {
        return 1;
    };
    if let Some(candidate) = remaining.get(..captured.len())
        && (reference.ascii || captured.is_ascii() && candidate.is_ascii())
    {
        state.end = block.current + captured.len();
        return i32::from(!captured.eq_ignore_ascii_case(candidate));
    }
    if reference.ascii {
        return 1;
    }
    // Backtracking often proposes captures longer than the remaining input; reject before comparing.
    let characters = state.character_position(end) - state.character_position(start);
    if characters
        > state.character_position(state.subject.len()) - state.character_position(block.current)
    {
        return 1;
    }
    let mut input = remaining.char_indices();
    let mut end = block.current;
    for expected in captured.chars() {
        let Some((offset, actual)) = input.next() else {
            return 1;
        };
        if lower(expected, reference.ascii) != lower(actual, reference.ascii) {
            return 1;
        }
        end = block.current + offset + actual.len_utf8();
    }
    // The atomic wrapper retains this offset until its end callout succeeds.
    state.end = end;
    0
}

#[cfg(test)]
mod tests {
    use super::super::Pattern;
    #[test]
    fn scoped_references_use_python_lowercase() {
        for (pattern, text, expected) in [
            (r"(?i)(.)\1", "Sſ", false),
            (r"(?i)(.)\1", "Σς", false),
            (r"(?i)(.)\1", "İi", true),
            (r"(?i)(.)\1", "𖺠𖺻", true),
            (r"(?ai)(.)\1(?ui:(.)\2)", "aAéÉ", true),
            (r"(?ai)(.)\1(?ui:(.)\2)", "éÉaA", false),
            (r"(?i)(.)\1(?<=\1\1)", "éÉ", true),
            (r"(?i)(.)?(?(1)\1|b)", "b", true),
            (r"(?i)((.)\2)+", "éÉéÉ", true),
            (r"(?i)()\1(?<=\1)é", "é", true),
            (r"(?i)(\s)\1", "é\n\n", true),
            (r"(?i)([\s\S]+)\1", "é\nÉ\n", true),
            (r"(?i)(.)\1", "é\0\0", true),
        ] {
            assert_eq!(
                Pattern::new(pattern)
                    .unwrap()
                    .unwrap()
                    .is_match(text)
                    .unwrap(),
                expected,
                "{pattern}: {text}"
            );
        }
    }
    #[test]
    fn independent_matches_share_only_compiled_code() {
        let pattern = Pattern::new(r"(?i)^(.+)\1$").unwrap().unwrap();
        std::thread::scope(|scope| {
            for _ in 0..4 {
                let pattern = &pattern;
                scope.spawn(move || {
                    for _ in 0..100 {
                        assert!(pattern.is_match("aéAÉ").unwrap());
                        assert!(!pattern.is_match("Sſ").unwrap());
                    }
                });
            }
        });
    }
    #[test]
    fn long_greedy_captures_reject_impossible_lengths() {
        let text = format!("{}é", "a".repeat(32_768));
        let doubled = format!("{text}{text}");
        let pattern = Pattern::new(r"(?i)^(.+)\1$").unwrap().unwrap();
        assert!(pattern.is_match(&doubled).unwrap());
        assert!(!pattern.is_match(&format!("{doubled}x")).unwrap());
    }
}
