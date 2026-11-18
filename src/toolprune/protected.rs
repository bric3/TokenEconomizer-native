use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Default, Serialize, Deserialize)]
pub struct ProtectedTools {
    pub names: BTreeSet<String>,
    #[serde(default)]
    pub patterns: Vec<String>,
}
impl ProtectedTools {
    pub fn contains(&self, name: &str) -> bool {
        self.names.contains(name)
            || self
                .patterns
                .iter()
                .any(|pattern| fnmatchcase(name, pattern))
    }
}

enum Token {
    Star,
    Any,
    Literal(u32),
    Class {
        negated: bool,
        ranges: Vec<(u32, u32)>,
    },
}
impl Token {
    fn matches(&self, c: u32) -> bool {
        match self {
            Self::Any => true,
            Self::Literal(literal) => *literal == c,
            Self::Class { negated, ranges } => {
                ranges.iter().any(|(lo, hi)| *lo <= c && c <= *hi) != *negated
            }
            Self::Star => unreachable!(),
        }
    }
}

pub fn fnmatchcase(name: &str, pattern: &str) -> bool {
    fnmatch_points(
        &name.chars().map(u32::from).collect::<Vec<_>>(),
        &pattern.chars().map(u32::from).collect::<Vec<_>>(),
    )
}

pub(crate) fn fnmatch_points(name: &[u32], pattern: &[u32]) -> bool {
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < pattern.len() {
        let c = pattern[i];
        i += 1;
        tokens.push(match char::from_u32(c) {
            Some('*') => {
                while i < pattern.len() && pattern[i] == u32::from('*') {
                    i += 1;
                }
                Token::Star
            }
            Some('?') => Token::Any,
            Some('[') => {
                let negated = pattern.get(i) == Some(&u32::from('!'));
                let start = i + usize::from(negated);
                let mut end = start + usize::from(pattern.get(start) == Some(&u32::from(']')));
                while end < pattern.len() && pattern[end] != u32::from(']') {
                    end += 1;
                }
                if end == pattern.len() {
                    Token::Literal(u32::from('['))
                } else {
                    let mut at = start;
                    let mut ranges = Vec::new();
                    while at < end {
                        if at + 2 < end && pattern[at + 1] == u32::from('-') {
                            // Python discards descending ranges, including both endpoints.
                            if pattern[at] <= pattern[at + 2] {
                                ranges.push((pattern[at], pattern[at + 2]));
                            }
                            at += 3;
                        } else {
                            ranges.push((pattern[at], pattern[at]));
                            at += 1;
                        }
                    }
                    i = end + 1;
                    Token::Class { negated, ranges }
                }
            }
            _ => Token::Literal(c),
        });
    }
    let mut matched = vec![false; name.len() + 1];
    matched[0] = true;
    for token in tokens {
        if matches!(token, Token::Star) {
            for j in 1..=name.len() {
                matched[j] |= matched[j - 1];
            }
        } else {
            for j in (1..=name.len()).rev() {
                matched[j] = matched[j - 1] && token.matches(name[j - 1]);
            }
            matched[0] = false;
        }
    }
    matched[name.len()]
}
