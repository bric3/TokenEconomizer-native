use super::{
    BLOCKED, Event, Policy, bounded_push, command_fingerprint, failure_signatures,
    green_is_trustworthy, is_research_command, is_verification_command, verification_scopes,
};
use crate::value::number;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Serialize, Deserialize)]
pub struct State {
    pub turns: i64,
    pub command_turns: i64,
    pub edit_events: i64,
    pub edited_paths: BTreeMap<String, i64>,
    pub last_edit_turn: i64,
    pub last_green_turn: i64,
    pub last_progress_turn: i64,
    pub last_failed_verification_turn: i64,
    pub verification_runs: i64,
    pub post_green_research: i64,
    pub failed_verifications_since_green: i64,
    pub failed_verifications_after_edit: i64,
    pub failure_sigs_since_green: BTreeMap<String, i64>,
    pub recent_commands: VecDeque<String>,
    pub recent_blocked: VecDeque<bool>,
    pub recent_window: usize,
    pub thread_id: String,
    pub last_decision_reason: String,
    pub first_trustworthy_green_turn: i64,
    pub last_green_trustworthy: bool,
    pub outstanding_failure_scopes: BTreeSet<String>,
    pub green_reruns_since_edit: i64,
}
impl State {
    pub fn new(recent_window: i64) -> Result<Self, String> {
        let recent_window = usize::try_from(recent_window)
            .map_err(|_| "ValueError: maxlen must be non-negative")?;
        Ok(Self {
            turns: 0,
            command_turns: 0,
            edit_events: 0,
            edited_paths: BTreeMap::new(),
            last_edit_turn: 0,
            last_green_turn: 0,
            last_progress_turn: 0,
            last_failed_verification_turn: 0,
            verification_runs: 0,
            post_green_research: 0,
            failed_verifications_since_green: 0,
            failed_verifications_after_edit: 0,
            failure_sigs_since_green: BTreeMap::new(),
            recent_commands: VecDeque::new(),
            recent_blocked: VecDeque::new(),
            recent_window,
            thread_id: String::new(),
            last_decision_reason: String::new(),
            first_trustworthy_green_turn: 0,
            last_green_trustworthy: false,
            outstanding_failure_scopes: BTreeSet::new(),
            green_reruns_since_edit: 0,
        })
    }
    pub fn distinct_edit_files(&self) -> i64 {
        self.edited_paths.len() as i64
    }
    pub fn edit_churn(&self) -> i64 {
        self.edited_paths.values().copied().max().unwrap_or(0)
    }
    pub fn green_after_edit(&self) -> bool {
        self.last_edit_turn != 0 && self.last_green_turn > self.last_edit_turn
    }
    pub fn currently_green(&self) -> bool {
        self.last_green_turn > self.last_edit_turn.max(self.last_failed_verification_turn)
    }
    pub fn repeated_failure_count(&self) -> i64 {
        self.failure_sigs_since_green
            .values()
            .copied()
            .max()
            .unwrap_or(0)
    }
    pub fn update(&mut self, event: &Event) {
        if event.kind == "thread" {
            if !event.thread_id.is_empty() {
                self.thread_id = event.thread_id.clone();
            }
            return;
        }
        if !["command", "edit"].contains(&event.kind.as_str()) {
            return;
        }
        self.turns += 1;
        if event.kind == "edit" {
            self.edit_events += 1;
            self.last_edit_turn = self.turns;
            self.last_progress_turn = self.turns;
            self.failed_verifications_after_edit = 0;
            self.post_green_research = 0;
            self.green_reruns_since_edit = 0;
            for path in &event.paths {
                *self.edited_paths.entry(path.clone()).or_default() += 1;
            }
            bounded_push(
                &mut self.recent_commands,
                "<edit>".into(),
                self.recent_window,
            );
            bounded_push(&mut self.recent_blocked, false, self.recent_window);
            return;
        }
        self.command_turns += 1;
        bounded_push(
            &mut self.recent_commands,
            command_fingerprint(&event.command),
            self.recent_window,
        );
        bounded_push(
            &mut self.recent_blocked,
            BLOCKED.is_match(&event.output),
            self.recent_window,
        );
        if !is_verification_command(&event.command) {
            if self.currently_green() && is_research_command(&event.command) {
                self.post_green_research += 1;
            }
            return;
        }
        self.verification_runs += 1;
        let filtered = !green_is_trustworthy(&event.command);
        let scopes = verification_scopes(&event.command);
        if number(&event.exit_code) == Some(0.0) {
            if self.green_after_edit() {
                self.green_reruns_since_edit += 1;
            }
            self.last_green_turn = self.turns;
            self.last_progress_turn = self.turns;
            self.failed_verifications_since_green = 0;
            self.failed_verifications_after_edit = 0;
            self.failure_sigs_since_green.clear();
            self.post_green_research = 0;
            if !filtered && let Some(scopes) = scopes {
                if scopes.contains("__ALL__") {
                    self.outstanding_failure_scopes.clear();
                } else {
                    self.outstanding_failure_scopes
                        .retain(|scope| !scopes.contains(scope));
                }
            }
            let trustworthy = self.last_edit_turn != 0 && !filtered;
            self.last_green_trustworthy = trustworthy;
            if trustworthy && self.first_trustworthy_green_turn == 0 {
                self.first_trustworthy_green_turn = self.turns;
            }
            return;
        }
        if event.exit_code.is_null() {
            return;
        }
        self.last_failed_verification_turn = self.turns;
        self.post_green_research = 0;
        self.failed_verifications_since_green += 1;
        if self.last_edit_turn != 0 {
            self.failed_verifications_after_edit += 1;
        }
        if !filtered && let Some(scopes) = scopes {
            self.outstanding_failure_scopes.extend(scopes);
        }
        for signature in failure_signatures(&event.output) {
            *self.failure_sigs_since_green.entry(signature).or_default() += 1;
        }
    }
    pub fn decision(&self, policy: &Policy) -> Option<&'static str> {
        if self.turns < policy.min_tool_turns {
            return None;
        }
        if policy.post_green_finalize_enabled {
            let blocked = policy.post_green_finalize_require_cleared_failures
                && !self.outstanding_failure_scopes.is_empty();
            if !blocked
                && self.currently_green()
                && self.last_green_trustworthy
                && self.first_trustworthy_green_turn != 0
                && self.turns - self.first_trustworthy_green_turn
                    >= policy.post_green_finalize_grace_turns.max(0)
            {
                return Some("validated-patch-should-finalize");
            }
        }
        if policy.reverify_loop_enabled
            && self.currently_green()
            && self.green_reruns_since_edit >= policy.reverify_loop_count
        {
            return Some("green-reverification-loop");
        }
        if policy.runaway_enabled
            && self.turns
                >= policy.runaway_floor.max(
                    (policy.runaway_multiplier * policy.min_tool_turns as f64).round_ties_even()
                        as i64,
                )
        {
            return Some("beyond-twice-the-resolved-envelope");
        }
        if policy.effort_rescue_enabled && !self.currently_green() {
            if self.last_progress_turn != 0
                && self.turns - self.last_progress_turn
                    < policy.effort_rescue_progress_grace_turns.max(0)
            {
                return None;
            }
            return Some("medium-effort-envelope-rescue");
        }
        if self.edited_paths.is_empty() {
            return None;
        }
        if policy.post_green_enabled && self.currently_green() {
            return (self.post_green_research >= policy.post_green_research_turns)
                .then_some("validated-patch-reopened-research");
        }
        if policy.scope_checkpoint_enabled
            && self.distinct_edit_files() >= policy.scope_checkpoint_files
        {
            return Some("wide-edit-scope-needs-verification");
        }
        if !policy.recovery_enabled
            || self.last_failed_verification_turn <= self.last_edit_turn
            || self.turns - self.last_failed_verification_turn < policy.recovery_failure_grace_turns
        {
            return None;
        }
        let failures = self.failed_verifications_since_green;
        let after_edit = self.failed_verifications_after_edit;
        if policy.repeated_failure_enabled
            && failures >= policy.min_failed_verifications
            && self.repeated_failure_count() >= policy.repeated_failure_count
        {
            return Some("repeated-failing-verification");
        }
        if policy.wide_failure_enabled
            && self.distinct_edit_files() >= policy.scatter_files
            && after_edit >= policy.min_failed_verifications
        {
            return Some("very-wide-patch-still-failing");
        }
        if policy.churn_failure_enabled
            && self.edit_churn() >= policy.churn_edits
            && after_edit >= policy.min_failed_verifications
            && self.repeated_failure_count() >= policy.repeated_failure_count
        {
            return Some("edit-churn-still-failing");
        }
        let no_progress = self.turns - self.last_progress_turn;
        let mut commands = BTreeMap::new();
        for command in self.recent_commands.iter().filter(|c| *c != "<edit>") {
            *commands.entry(command).or_insert(0_i64) += 1;
        }
        let same_command = commands.values().copied().max().unwrap_or(0);
        if policy.repeated_command_enabled
            && same_command >= policy.repeated_command_count
            && no_progress >= policy.no_progress_turns
            && failures != 0
        {
            return Some("repeated-command-without-progress");
        }
        if policy.blocked_retrieval_enabled
            && (self.recent_blocked.iter().filter(|v| **v).count() as i64)
                >= policy.blocked_attempts
            && no_progress >= policy.no_progress_turns
            && failures != 0
        {
            return Some("blocked-retrieval-without-progress");
        }
        None
    }
    pub fn snapshot(&self) -> Value {
        json!({"tool_turns":self.turns,"command_turns":self.command_turns,"edit_events":self.edit_events,
            "distinct_edit_files":self.distinct_edit_files(),"edit_churn":self.edit_churn(),"last_edit_turn":self.last_edit_turn,
            "last_green_turn":self.last_green_turn,"last_failed_verification_turn":self.last_failed_verification_turn,
            "failed_verifications_since_green":self.failed_verifications_since_green,"failed_verifications_after_edit":self.failed_verifications_after_edit,
            "repeated_failure_count":self.repeated_failure_count(),"green_after_edit":self.green_after_edit(),"currently_green":self.currently_green(),
            "post_green_research":self.post_green_research,"verification_runs":self.verification_runs,"first_trustworthy_green_turn":self.first_trustworthy_green_turn,
            "last_green_trustworthy":self.last_green_trustworthy,"outstanding_failure_scopes":self.outstanding_failure_scopes,"green_reruns_since_edit":self.green_reruns_since_edit})
    }
}
