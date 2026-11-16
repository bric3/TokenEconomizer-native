use std::collections::HashSet;

use serde::{Deserialize, Serialize};

pub const K_PROMOTE: usize = 3;
pub const GAMMA: f64 = 0.8;
pub const LAMBDA: f64 = 5.0;
pub const SCORE_FLOOR: f64 = 0.0;
pub const FIRE_GATE: f64 = 0.40;
pub const FIRE_MIN_N: u64 = 4;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Lifecycle {
    pub key: String,
    pub high: bool,
    #[serde(default = "default_drift")]
    pub drift: bool,
    pub state: String,
    pub confirms: Vec<(i64, Vec<String>)>,
    pub contradicts: Vec<i64>,
    pub fire_num: u64,
    pub fire_den: u64,
    pub promoted_at: Option<i64>,
    pub retired_at: Option<i64>,
}

fn default_drift() -> bool {
    true
}

impl Lifecycle {
    pub fn new(key: String, high: bool, drift: bool) -> Self {
        Self {
            key,
            high,
            drift,
            state: "CANDIDATE".into(),
            confirms: Vec::new(),
            contradicts: Vec::new(),
            fire_num: 0,
            fire_den: 0,
            promoted_at: None,
            retired_at: None,
        }
    }

    pub fn drifted_confirms(&self) -> usize {
        if !self.drift {
            return self.confirms.len();
        }
        let mut kept = Vec::<HashSet<&String>>::new();
        for (_, files) in &self.confirms {
            let files = files.iter().collect::<HashSet<_>>();
            if files.is_empty() || kept.iter().all(|prior| prior.is_disjoint(&files)) {
                kept.push(files);
            }
        }
        kept.len()
    }

    pub fn score(&self, now: i64) -> f64 {
        self.confirms
            .iter()
            .map(|(i, _)| GAMMA.powf(now as f64 - *i as f64))
            .sum::<f64>()
            - LAMBDA
                * self
                    .contradicts
                    .iter()
                    .map(|i| GAMMA.powf(now as f64 - *i as f64))
                    .sum::<f64>()
    }

    pub fn firing_rate(&self) -> f64 {
        if self.fire_den == 0 {
            0.0
        } else {
            self.fire_num as f64 / self.fire_den as f64
        }
    }

    pub fn injectable(&self) -> bool {
        self.state == "ACTIVE" && self.fire_den >= FIRE_MIN_N && self.firing_rate() >= FIRE_GATE
    }

    pub fn update(
        &mut self,
        now: i64,
        verdict: &str,
        fired: bool,
        exercised: bool,
        mut files: Vec<String>,
    ) {
        if exercised {
            self.fire_den += 1;
            if fired {
                self.fire_num += 1;
            }
        }
        match verdict {
            "confirm" => {
                files.sort();
                self.confirms.push((now, files));
            }
            "contradict" => self.contradicts.push(now),
            _ => (),
        }
        if self.state == "CANDIDATE" {
            if self.drifted_confirms() >= K_PROMOTE {
                self.state = "ACTIVE".into();
                self.promoted_at = Some(now);
            }
        } else if self.state == "ACTIVE"
            && ((self.high && verdict == "contradict") || self.score(now) < SCORE_FLOOR)
        {
            self.state = "RETIRED".into();
            self.retired_at = Some(now);
        }
    }
}
