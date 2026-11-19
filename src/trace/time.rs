// Parsing and local-time mapping follow CPython 3.15.2's _datetimemodule.c.
// The Python Software Foundation licence is preserved in LICENSE-CPYTHON.
use std::{
    borrow::Cow,
    time::{SystemTime, UNIX_EPOCH},
};

use chrono::{Datelike, Local, NaiveDate, TimeZone, Weekday};
use num_rational::Ratio;
use num_traits::ToPrimitive;
use serde_json::Value;

pub fn system_seconds(time: SystemTime) -> f64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs_f64(),
        Err(error) => -error.duration().as_secs_f64(),
    }
}

fn digits(text: &[u8], pos: &mut usize, width: usize) -> Option<u32> {
    let mut value = 0;
    for &digit in text.get(*pos..*pos + width)? {
        if !digit.is_ascii_digit() {
            return None;
        }
        value = value * 10 + u32::from(digit - b'0');
    }
    *pos += width;
    Some(value)
}

// Python resolves week-date ambiguity before consuming the single Unicode separator.
fn date_end(text: &[u8]) -> Option<usize> {
    if text.len() < 7 {
        return None;
    }
    if text.len() == 7 {
        return Some(7);
    }
    if text[4] == b'-' {
        if text[5] == b'W' {
            if text.get(8) == Some(&b'-') {
                if text.len() == 9 {
                    return None;
                }
                if text.get(10).is_some_and(u8::is_ascii_digit) {
                    Some(8)
                } else {
                    Some(10)
                }
            } else {
                Some(8)
            }
        } else {
            Some(10)
        }
    } else if text[4] == b'W' {
        let mut end = 7;
        while text.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
        Some(if end < 9 {
            end
        } else if end % 2 == 0 {
            7
        } else {
            8
        })
    } else {
        Some(8)
    }
}

fn date(text: &[u8]) -> Option<(i32, u32, u32)> {
    let mut pos = 0;
    let year = digits(text, &mut pos, 4)? as i32;
    let separated = text.get(pos) == Some(&b'-');
    pos += usize::from(separated);
    if text.get(pos) == Some(&b'W') {
        pos += 1;
        let week = digits(text, &mut pos, 2)?;
        let day = if pos < text.len() {
            if separated {
                if text.get(pos) != Some(&b'-') {
                    return None;
                }
                pos += 1;
            }
            digits(text, &mut pos, 1)?
        } else {
            1
        };
        if !(1..=9999).contains(&year) || !(1..=7).contains(&day) {
            return None;
        }
        let weekday = [
            Weekday::Mon,
            Weekday::Tue,
            Weekday::Wed,
            Weekday::Thu,
            Weekday::Fri,
            Weekday::Sat,
            Weekday::Sun,
        ][day as usize - 1];
        let date = NaiveDate::from_isoywd_opt(year, week, weekday)?;
        Some((date.year(), date.month(), date.day()))
    } else {
        let month = digits(text, &mut pos, 2)?;
        if separated {
            if text.get(pos) != Some(&b'-') {
                return None;
            }
            pos += 1;
        }
        let day = digits(text, &mut pos, 2)?;
        Some((year, month, day))
    }
}

#[derive(Default)]
struct Clock {
    hour: u32,
    minute: u32,
    second: u32,
    microsecond: u32,
}
impl Clock {
    fn valid(&self) -> bool {
        self.hour < 24 && self.minute < 60 && self.second < 60
    }
    fn micros(&self) -> i64 {
        i64::from(self.hour * 3600 + self.minute * 60 + self.second) * 1_000_000
            + i64::from(self.microsecond)
    }
}

fn byte(text: &[u8], pos: usize) -> u8 {
    text.get(pos).copied().unwrap_or(0)
}

// The C parser reports trailing data separately because an offset follows the clock.
fn clock(text: &[u8], mut pos: usize, end: usize) -> Option<(Clock, bool)> {
    let mut values = [0; 4];
    let mut separated = false;
    for index in 0..3 {
        values[index] = digits(text, &mut pos, 2)?;
        let next = byte(text, pos);
        pos += 1;
        if index == 0 {
            separated = next == b':';
        }
        if matches!(next, b'.' | b',') {
            if index < 2 || pos >= end {
                return None;
            }
            break;
        } else if pos >= end {
            return Some((
                Clock {
                    hour: values[0],
                    minute: values[1],
                    second: values[2],
                    microsecond: 0,
                },
                next != 0,
            ));
        } else if separated && next == b':' {
            if index == 2 {
                return None;
            }
        } else if !separated {
            pos -= 1;
        } else {
            return None;
        }
    }
    let width = end.checked_sub(pos)?.min(6);
    if width == 0 {
        return None;
    }
    values[3] = digits(text, &mut pos, width)? * 10_u32.pow(6 - width as u32);
    while byte(text, pos).is_ascii_digit() {
        pos += 1;
    }
    Some((
        Clock {
            hour: values[0],
            minute: values[1],
            second: values[2],
            microsecond: values[3],
        },
        byte(text, pos) != 0,
    ))
}

fn time(text: &[u8]) -> Option<(Clock, Option<i64>)> {
    let zone = text
        .iter()
        .position(|c| matches!(c, b'+' | b'-' | b'Z'))
        .unwrap_or(text.len());
    let (value, trailing) = clock(text, 0, zone)?;
    if zone == text.len() {
        return (!trailing).then_some((value, None));
    }
    if text[zone] == b'Z' {
        return (byte(text, zone + 1) == 0).then_some((value, Some(0)));
    }
    let (offset, trailing) = clock(text, zone + 1, text.len())?;
    if trailing || !offset.valid() {
        return None;
    }
    let offset = offset.micros() * if text[zone] == b'-' { -1 } else { 1 };
    Some((value, Some(offset)))
}

fn local(seconds: i64) -> Option<i64> {
    let local = Local.timestamp_opt(seconds, 0).single()?.naive_local();
    (1..=9999)
        .contains(&local.year())
        .then(|| local.and_utc().timestamp())
}

// Solve Python's fold=0 wall-time mapping, including DST gaps and overlaps.
fn local_seconds(wall: i64) -> Option<i64> {
    let a = local(wall)? - wall;
    let u1 = wall - a;
    let t1 = local(u1)?;
    let b = if t1 == wall {
        let u2 = u1 - 86400;
        let b = local(u2)? - u2;
        if a == b {
            return Some(u1);
        }
        b
    } else {
        t1 - u1
    };
    let u2 = wall - b;
    let t2 = local(u2)?;
    Some(if t2 == wall {
        u2
    } else if t1 == wall {
        u1
    } else {
        u1.max(u2)
    })
}

pub fn epoch(value: &Value) -> Option<f64> {
    let text = value.as_str().filter(|text| !text.is_empty())?;
    let text = if let Some(prefix) = text.strip_suffix('Z') {
        Cow::Owned(format!("{prefix}+00:00"))
    } else {
        Cow::Borrowed(text)
    };
    let end = date_end(text.as_bytes())?;
    let (mut year, mut month, mut day) = date(text.as_bytes().get(..end)?)?;
    let (mut clock, offset) = if end < text.len() {
        let separator = text.get(end..)?.chars().next()?.len_utf8();
        time(text.as_bytes().get(end + separator..)?)?
    } else {
        (Clock::default(), None)
    };
    // Python normalises 24:00 before validating the resulting calendar date.
    if clock.hour == 24 && clock.minute == 0 && clock.second == 0 && clock.microsecond == 0 {
        let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let max = match month {
            2 => 28 + u32::from(leap),
            4 | 6 | 9 | 11 => 30,
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            _ => return None,
        };
        if day <= max {
            clock.hour = 0;
            day += 1;
            if day > max {
                day = 1;
                month += 1;
                if month > 12 {
                    month = 1;
                    year += 1;
                }
            }
        }
    }
    if !(1..=9999).contains(&year) || !clock.valid() {
        return None;
    }
    let naive = NaiveDate::from_ymd_opt(year, month, day)?.and_hms_micro_opt(
        clock.hour,
        clock.minute,
        clock.second,
        clock.microsecond,
    )?;
    if let Some(offset) = offset {
        // Aware datetimes divide integer microseconds once; this avoids double rounding.
        Ratio::new_raw(naive.and_utc().timestamp_micros() - offset, 1_000_000).to_f64()
    } else {
        Some(
            local_seconds(naive.and_utc().timestamp())? as f64 + f64::from(clock.microsecond) / 1e6,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::epoch;
    use serde_json::json;

    #[test]
    fn iso_week_midnight_and_fractional_offset_timestamps() {
        for (text, expected) in [
            ("1970-W01-4T00:00:01+00", 1.0),
            ("19700101😀000001+00", 1.0),
            ("2024-02-29T24:00:00Z", 1709251200.0),
            ("1970-01-01T00:00:01+00:00:00.5", 0.5),
            ("1969-12-31T23:59:59.999999Z", -0.000001),
            ("9999-12-31T23:59:59.999999Z", 253402300800.0),
        ] {
            assert_eq!(epoch(&json!(text)), Some(expected), "{text}");
        }
    }

    #[test]
    fn malformed_fields_and_out_of_range_clocks_are_rejected() {
        for text in [
            " 2025-01-01",
            "2025-01-01T1:00",
            "2025-01-01T12.5",
            "2025-01-01T12:34.5",
            "2025-01-01T23:59:60",
            "2025-01-01T24:00:00.000001",
            "2025-01-01T00:00+24:00",
            "2025-01-01T00:00+00:00:60",
            "9999-12-31T24:00Z",
        ] {
            assert_eq!(epoch(&json!(text)), None, "{text}");
        }
    }
}
