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
    Literal(char),
    Class {
        negated: bool,
        ranges: Vec<(char, char)>,
    },
}
impl Token {
    fn matches(&self, c: char) -> bool {
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
    let pattern: Vec<_> = pattern.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < pattern.len() {
        let c = pattern[i];
        i += 1;
        tokens.push(match c {
            '*' => {
                while i < pattern.len() && pattern[i] == '*' {
                    i += 1;
                }
                Token::Star
            }
            '?' => Token::Any,
            '[' => {
                let negated = pattern.get(i) == Some(&'!');
                let start = i + usize::from(negated);
                let mut end = start + usize::from(pattern.get(start) == Some(&']'));
                while end < pattern.len() && pattern[end] != ']' {
                    end += 1;
                }
                if end == pattern.len() {
                    Token::Literal('[')
                } else {
                    let mut at = start;
                    let mut ranges = Vec::new();
                    while at < end {
                        if at + 2 < end && pattern[at + 1] == '-' {
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
            c => Token::Literal(c),
        });
    }
    let name: Vec<_> = name.chars().collect();
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
