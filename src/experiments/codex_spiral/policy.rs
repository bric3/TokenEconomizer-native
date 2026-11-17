use serde::{Deserialize, Serialize};

pub const PROFILE_VERSION: &str = "codex-native-spiral-post-green-cleared-scopes";
pub const DEFENSIVE_PROFILE_VERSION: &str = "codex-native-spiral-defensive-v1";
pub const CODEX_DEFAULT_PROFILE: &str = "defensive";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Policy {
    pub min_tool_turns: i64,
    pub min_failed_verifications: i64,
    pub repeated_failure_count: i64,
    pub scatter_files: i64,
    pub churn_edits: i64,
    pub recent_window: i64,
    pub repeated_command_count: i64,
    pub blocked_attempts: i64,
    pub no_progress_turns: i64,
    pub post_green_research_turns: i64,
    pub scope_checkpoint_files: i64,
    pub recovery_failure_grace_turns: i64,
    pub recovery_enabled: bool,
    pub post_green_enabled: bool,
    pub scope_checkpoint_enabled: bool,
    pub repeated_failure_enabled: bool,
    pub wide_failure_enabled: bool,
    pub churn_failure_enabled: bool,
    pub repeated_command_enabled: bool,
    pub blocked_retrieval_enabled: bool,
    pub effort_rescue_enabled: bool,
    pub effort_rescue_progress_grace_turns: i64,
    pub resume_effort: String,
    pub nudge_budget: i64,
    pub post_green_finalize_enabled: bool,
    pub post_green_finalize_grace_turns: i64,
    pub post_green_finalize_require_cleared_failures: bool,
    pub reverify_loop_enabled: bool,
    pub reverify_loop_count: i64,
    pub runaway_enabled: bool,
    pub runaway_multiplier: f64,
    pub runaway_floor: i64,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            min_tool_turns: 30,
            min_failed_verifications: 2,
            repeated_failure_count: 2,
            scatter_files: 15,
            churn_edits: 3,
            recent_window: 12,
            repeated_command_count: 3,
            blocked_attempts: 2,
            no_progress_turns: 8,
            post_green_research_turns: 4,
            scope_checkpoint_files: 8,
            recovery_failure_grace_turns: 0,
            recovery_enabled: true,
            post_green_enabled: false,
            scope_checkpoint_enabled: false,
            repeated_failure_enabled: false,
            wide_failure_enabled: true,
            churn_failure_enabled: false,
            repeated_command_enabled: false,
            blocked_retrieval_enabled: false,
            effort_rescue_enabled: false,
            effort_rescue_progress_grace_turns: 3,
            resume_effort: String::new(),
            nudge_budget: 1,
            post_green_finalize_enabled: false,
            post_green_finalize_grace_turns: 3,
            post_green_finalize_require_cleared_failures: false,
            reverify_loop_enabled: false,
            reverify_loop_count: 3,
            runaway_enabled: false,
            runaway_multiplier: 2.0,
            runaway_floor: 30,
        }
    }
}
impl Policy {
    pub fn at_threshold(&self, turns: f64) -> Self {
        Self {
            min_tool_turns: (turns.round_ties_even() as i64).max(1),
            ..self.clone()
        }
    }
    pub fn shipped() -> Self {
        Self {
            post_green_finalize_enabled: true,
            post_green_finalize_require_cleared_failures: true,
            wide_failure_enabled: false,
            recovery_enabled: false,
            ..Self::default()
        }
    }
    pub fn defensive() -> Self {
        Self {
            recovery_enabled: true,
            repeated_failure_enabled: true,
            wide_failure_enabled: true,
            churn_failure_enabled: true,
            repeated_command_enabled: true,
            blocked_retrieval_enabled: true,
            reverify_loop_enabled: true,
            runaway_enabled: true,
            nudge_budget: 2,
            ..Self::shipped()
        }
    }
}
pub fn profile_policy(profile: &str, turns: f64) -> Policy {
    (if profile == "defensive" {
        Policy::defensive()
    } else {
        Policy::shipped()
    })
    .at_threshold(turns)
}
pub fn profile_version(profile: &str) -> &'static str {
    if profile == "defensive" {
        DEFENSIVE_PROFILE_VERSION
    } else {
        PROFILE_VERSION
    }
}

pub fn nudge_text(reason: &str) -> String {
    let specific = match reason {
        "validated-patch-should-finalize" => {
            Some(include_str!("prompts/validated-patch-should-finalize.txt"))
        }
        "green-reverification-loop" => Some(include_str!("prompts/green-reverification-loop.txt")),
        "beyond-twice-the-resolved-envelope" => Some(include_str!(
            "prompts/beyond-twice-the-resolved-envelope.txt"
        )),
        "medium-effort-envelope-rescue" => {
            Some(include_str!("prompts/medium-effort-envelope-rescue.txt"))
        }
        "very-wide-patch-still-failing" => {
            Some(include_str!("prompts/very-wide-patch-still-failing.txt"))
        }
        "wide-edit-scope-needs-verification" => Some(include_str!(
            "prompts/wide-edit-scope-needs-verification.txt"
        )),
        "validated-patch-reopened-research" => Some(include_str!(
            "prompts/validated-patch-reopened-research.txt"
        )),
        _ => None,
    };
    if let Some(text) = specific {
        return text.into();
    }
    let focus = match reason {
        "repeated-failing-verification" => {
            "The same verification failure is recurring after attempted fixes."
        }
        "scattered-edits-still-failing" => {
            "The patch has spread across many files and verification is still failing."
        }
        "edit-churn-still-failing" => "Repeated edits have not changed the verification outcome.",
        "repeated-command-without-progress" => {
            "The same command pattern is recurring without a successful verification."
        }
        "blocked-retrieval-without-progress" => {
            "Blocked external retrieval is recurring without advancing the local fix."
        }
        _ => "The recent process is repeating without a successful verification.",
    };
    format!(
        "Automated process check: {focus}{}",
        include_str!("prompts/generic.txt")
    )
}
