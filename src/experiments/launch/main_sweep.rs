//! Run one agent/model/domain tree through the native planner and experiment driver.
use super::{Context, environment};
use crate::experiments::{CommandOutput, sweep};
use serde_json::{Map, Value};
use std::{fs, io, path::PathBuf};

impl Context {
    fn planned(&mut self, args: &[String]) -> CommandOutput {
        match sweep::command("te-bench sweep-plan", args, &self.cwd, &self.cwd) {
            Ok(output) => output,
            Err(error) => CommandOutput {
                returncode: Some(1),
                stderr: format!("{}\n", error.message),
                ..CommandOutput::default()
            },
        }
    }
    fn main_sweep(&mut self, args: &[String]) -> io::Result<()> {
        let python = self.setting("PYTHON", "python3").to_owned();
        self.set("REPO_ROOT", self.cwd.to_string_lossy().into_owned());
        self.load_env();
        self.code = 0;
        self.default("AGENT", "claude");
        self.default("MODEL", "claude-sonnet-5");
        let agent = self.get("AGENT").to_owned();
        let requested_model = self.get("MODEL").to_owned();
        let model = self.model_for_agent(&agent, &requested_model);
        self.set("MODEL", model.clone());
        self.flush()?;
        for (key, value) in [
            ("DOMAIN", "swebench"),
            ("SEEDS", "0 1 2"),
            ("LIMIT", "0"),
            ("RUNNER", "stub"),
            ("SKIP_DONE", "1"),
            ("DRYRUN", "0"),
            ("ON_FAIL", "continue"),
        ] {
            self.default(key, value);
        }
        let domain = self.get("DOMAIN").to_owned();
        if !matches!(domain.as_str(), "swebench" | "webbench") {
            self.err(format!(
                "!! main_sweep covers DOMAIN=swebench|webbench (got '{domain}')"
            ));
            self.stop(2);
            return Ok(());
        }
        let tasks_key = format!("TOKENECONOMIZER_{}_TASKS", domain.to_ascii_uppercase());
        let domain_tasks = self.get(&tasks_key).to_owned();
        self.default("TASKS_FILE", &domain_tasks);
        if self.get("TASKS_FILE").is_empty() && self.get("DRYRUN") != "1" {
            self.err(format!(
                "!! no task-spec JSON: set TASKS_FILE or {tasks_key}"
            ));
            self.stop(2);
            return Ok(());
        }
        let tag = environment::model_tag(&model);
        if tag.is_empty() {
            self.err(format!(
                "!! MODEL='{model}' is a router directive, not a pinned model: the tree would carry"
            ));
            self.err("!! no model tag and the effort arms could not pin effort. Pin an id (e.g.");
            self.err("!! MODEL=claude-sonnet-5 or MODEL=claude-opus-4-8).");
            self.stop(2);
            return Ok(());
        }
        let workload = self
            .setting(
                "WORKLOAD",
                &format!("{}/workloads/{domain}.json", self.cwd.display()),
            )
            .to_owned();
        let mut workload_args = Vec::new();
        if workload != "none" {
            if !self.cwd.join(&workload).is_file() {
                self.err(format!("!! workload manifest not found: {workload}"));
                self.stop(2);
                return Ok(());
            }
            workload_args.extend(["--workload".into(), workload.clone()]);
        }
        let results = self
            .setting(
                "TOKENECONOMIZER_RESULTS",
                &format!("{}/results", self.cwd.display()),
            )
            .to_owned();
        self.set("RESULTS_ROOT", results.clone());
        let dest = format!(
            "{results}/{}",
            environment::tree_name(&agent, &domain, &tag)
        );
        let mut plan_args = vec!["--domain".into(), domain.clone()];
        plan_args.extend(workload_args.iter().cloned());
        for (key, option) in [("ARMS", "--arms"), ("REPOS", "--repos")] {
            if !self.get(key).is_empty() {
                plan_args.push(option.into());
                plan_args.extend(
                    self.get(key)
                        .split([' ', '\t', '\n'])
                        .filter(|part| !part.is_empty())
                        .map(str::to_owned),
                );
            }
        }
        plan_args.push("--seeds".into());
        plan_args.extend(
            self.get("SEEDS")
                .split([' ', '\t', '\n'])
                .filter(|part| !part.is_empty())
                .map(str::to_owned),
        );
        let mut cells_args = plan_args.clone();
        cells_args.extend(["--format".into(), "cells".into()]);
        let planned = self.planned(&cells_args);
        self.stderr.push_str(&planned.stderr);
        if planned.exit.or(planned.returncode).unwrap_or(0) != 0 {
            self.err("!! could not plan the sweep (see the error above)");
            self.stop(2);
            return Ok(());
        }
        let cells: Vec<_> = planned.stdout.lines().map(str::to_owned).collect();
        if cells.is_empty() {
            self.err(format!(
                "!! the plan is empty: no runnable cell for arms='{}'",
                self.setting("ARMS", "<default>")
            ));
            self.err(format!(
                "!! repos='{}' seeds='{}' domain={domain}",
                self.setting("REPOS", "<all>"),
                self.get("SEEDS")
            ));
            self.stop(2);
            return Ok(());
        }
        self.flush()?;
        self.out(format!(
            "[main_sweep] agent={agent} model={model} tag={tag} domain={domain}"
        ));
        self.out(format!("[main_sweep] dest={dest}"));
        self.out(format!(
            "[main_sweep] cells={} seeds='{}' runner={} skip_done={}",
            cells.len(),
            self.get("SEEDS"),
            self.get("RUNNER"),
            self.get("SKIP_DONE")
        ));
        plan_args.extend([
            "--limit".into(),
            self.get("LIMIT").into(),
            "--format".into(),
            "estimate".into(),
        ]);
        let estimate = self.planned(&plan_args);
        self.stderr.push_str(&estimate.stderr);
        for line in estimate.stdout.lines() {
            self.out(format!("[main_sweep] {line}"));
        }
        self.flush()?;
        self.check_routing(&agent);
        if self.stopped {
            return Ok(());
        }
        if agent == "copilot" && self.get("RUNNER") != "stub" {
            let effort = self.setting("TE_EFFORT_LEVEL", "high").to_owned();
            self.flush()?;
            self.check_copilot_model(&model, &effort);
        }
        if self.stopped {
            return Ok(());
        }
        if self.get("DRYRUN") == "1" {
            self.out("[main_sweep] DRYRUN -- the cells this lane would run, in order:");
            for cell in cells {
                self.out(format!("  {cell}"));
            }
            self.out("[main_sweep] DRYRUN: nothing was run.");
            return Ok(());
        }
        fs::create_dir_all(self.cwd.join(&dest))?;
        self.flush()?;
        let executable = std::env::current_exe()?;
        let mut detached = vec![
            executable.to_string_lossy().into_owned(),
            "main-sweep".into(),
        ];
        detached.extend_from_slice(args);
        self.detach(&format!("main_sweep__{agent}_{tag}_{domain}"), &detached);
        if self.stopped {
            return Ok(());
        }
        self.run_cells(
            super::execution::Workflow::Main,
            &cells,
            &dest,
            &workload,
            &python,
        )
    }
}

pub fn command(args: &[String], env: Map<String, Value>, cwd: PathBuf) -> io::Result<i32> {
    let mut context = Context::new(env, cwd);
    let result = context.main_sweep(args);
    context.flush()?;
    result?;
    Ok(context.code)
}
