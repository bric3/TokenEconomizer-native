use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Datelike, Local, NaiveDate, NaiveDateTime, TimeDelta, TimeZone};
use serde_json::Value;

pub fn system_seconds(time: SystemTime) -> f64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs_f64(),
        Err(error) => -error.duration().as_secs_f64(),
    }
}

pub fn epoch(value: &Value) -> Option<f64> {
    let text = value.as_str().filter(|s| !s.is_empty())?;
    let aware = DateTime::parse_from_rfc3339(text).ok().or_else(|| {
        [
            "%Y-%m-%dT%H:%M:%S%.f%#z",
            "%Y-%m-%d %H:%M:%S%.f%#z",
            "%Y%m%dT%H%M%S%.f%#z",
        ]
        .iter()
        .find_map(|format| DateTime::parse_from_str(text, format).ok())
    });
    if let Some(time) = aware {
        if !(1..=9999).contains(&time.year()) || time.timestamp_subsec_nanos() >= 1_000_000_000 {
            return None;
        }
        return Some(time.timestamp() as f64 + time.timestamp_subsec_micros() as f64 / 1e6);
    }
    let naive = [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%dT%H",
        "%Y%m%dT%H%M%S%.f",
    ]
    .iter()
    .find_map(|format| NaiveDateTime::parse_from_str(text, format).ok())
    .or_else(|| {
        ["%Y-%m-%d", "%Y%m%d", "%G-W%V-%u"]
            .iter()
            .find_map(|format| NaiveDate::parse_from_str(text, format).ok())
            .and_then(|date| date.and_hms_opt(0, 0, 0))
    })?;
    if !(1..=9999).contains(&naive.year())
        || naive.and_utc().timestamp_subsec_nanos() >= 1_000_000_000
    {
        return None;
    }
    let time = Local
        .from_local_datetime(&naive)
        .earliest()
        .map(|time| time.fixed_offset())
        .or_else(|| {
            // Python's fold=0 uses the earlier offset for a local time inside a DST gap.
            let prior = naive.checked_sub_signed(TimeDelta::days(2))?;
            let offset = *Local.from_local_datetime(&prior).earliest()?.offset();
            offset.from_local_datetime(&naive).earliest()
        })?;
    Some(time.timestamp() as f64 + time.timestamp_subsec_micros() as f64 / 1e6)
}
