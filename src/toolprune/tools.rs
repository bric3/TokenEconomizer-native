use serde_json::{Map, Value};

use crate::{
    pricing::approx_tokens,
    runner::Runner,
    value::{scalar_text, truthy},
};

pub const INAPPLICABLE_TOOLS: &[&str] = &[
    "task",
    "list_agents",
    "read_agent",
    "write_agent",
    "sql",
    "session_store_sql",
    "manage_schedule",
    "skill",
    "fetch_copilot_cli_documentation",
    "store_memory",
    "vote_memory",
    "ask_user",
    "Agent",
    "Workflow",
    "TaskCreate",
    "TaskUpdate",
    "TaskGet",
    "TaskList",
    "TaskOutput",
    "TaskStop",
    "ScheduleWakeup",
    "CronCreate",
    "CronDelete",
    "CronList",
    "EnterPlanMode",
    "ExitPlanMode",
    "EnterWorktree",
    "ExitWorktree",
    "Skill",
    "NotebookEdit",
    "AskUserQuestion",
];
pub const PROTECTED_TOOLS: &[&str] = &[
    "bash",
    "view",
    "grep",
    "glob",
    "edit",
    "create",
    "str_replace",
    "read_bash",
    "stop_bash",
    "list_bash",
    "read_file",
    "shell",
    "apply_patch",
    "Bash",
    "Edit",
    "Write",
    "Read",
    "Grep",
    "Glob",
    "MultiEdit",
];
pub const TOOL_JUDGE_DOMAIN: &str = include_str!("prompts/judge.txt");

pub fn judge_tools_for_task(
    problem: &str,
    catalog: &Map<String, Value>,
    runner: Option<&mut dyn Runner>,
    model: &str,
    domain: Option<&str>,
    max_issue_chars: i64,
) -> (Map<String, Value>, f64) {
    let Some(runner) = runner.filter(|_| !catalog.is_empty() && !problem.is_empty()) else {
        return (Map::new(), 0.0);
    };
    let length = problem.chars().count() as i64;
    let cap = if max_issue_chars < 0 {
        (length + max_issue_chars).max(0)
    } else {
        max_issue_chars
    };
    let prompt = format!(
        "{}\nISSUE:\n{}\n\nTOOLS:\n{}\n\nReturn the JSON classification:",
        domain
            .filter(|d| !d.is_empty())
            .unwrap_or(TOOL_JUDGE_DOMAIN),
        problem.chars().take(cap as usize).collect::<String>(),
        catalog
            .iter()
            .map(|(name, desc)| format!("- {name}: {}", scalar_text(desc)))
            .collect::<Vec<_>>()
            .join("\n")
    );
    let Ok((raw, effective)) = runner.run(&prompt, model) else {
        return (Map::new(), 0.0);
    };
    let tokens =
        effective.unwrap_or_else(|| (approx_tokens(&prompt) + 5 * approx_tokens(&raw)) as f64);
    let mut out = Map::new();
    if let (Some(start), Some(end)) = (raw.find('{'), raw.rfind('}'))
        && start <= end
    {
        let Ok(Value::Object(object)) = serde_json::from_str::<Value>(&raw[start..=end]) else {
            return (out, tokens);
        };
        for (tool, value) in object {
            let load = if value.is_object() {
                &value["load"]
            } else {
                &value
            };
            out.insert(
                tool.clone(),
                Value::Bool(PROTECTED_TOOLS.contains(&tool.as_str()) || truthy(load)),
            );
        }
    }
    (out, tokens)
}
