//! Launch the paired or explicitly selected Codex experiment arms.
use super::{Context, environment, execution::Workflow};
use crate::experiments::sweep;
use serde_json::{Map, Value};
use std::{fs, io, path::PathBuf};

const ARMS: &str = concat!(
    "baseline baseline-medium baseline-low repomap rtk serena codex-untool codex-spiral ",
    "codex-spiral-postgreen codex-spiral-defensive codex-combo codex-trimech codex-trimech-rtk ",
    "codex-trimech-repomap codex-inject codex-inject-untool codex-inject-envfacts codex-inject-distiller"
);

fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split([' ', '\t', '\n'])
        .filter(|part| !part.is_empty())
}

impl Context {
    fn codex_arm(&mut self, arm: &str) -> bool {
        // Bash checks a delimited substring, including adjacent names in a compound ARM value.
        if format!(" {ARMS} ").contains(&format!(" {arm} ")) {
            return true;
        }
        self.err(format!("!! unknown Codex arm '{arm}'; one of: {ARMS}"));
        self.stop(2);
        false
    }
    fn codex_ab(&mut self, args: &[String]) -> io::Result<()> {
        let python = self.setting("PYTHON", "python3").to_owned();
        self.set("REPO_ROOT", self.cwd.to_string_lossy().into_owned());
        self.load_env();
        self.code = 0;
        self.export(
            "TE_SWEBENCH_MAX_CPUS",
            self.setting("TE_SWEBENCH_MAX_CPUS", "4").to_owned(),
        );
        let (arms, lane_tag) = if !self.get("ARMS").is_empty() {
            // Bash read -a consumes one line; REPOS and SEEDS use unquoted word splitting.
            let arms: Vec<String> = words(self.get("ARMS").split('\n').next().unwrap())
                .map(str::to_owned)
                .collect();
            for arm in &arms {
                if !self.codex_arm(arm) {
                    return Ok(());
                }
            }
            (arms, "arms".to_owned())
        } else {
            let arm = self.setting("ARM", "codex-untool").to_owned();
            if !self.codex_arm(&arm) {
                return Ok(());
            }
            match self.setting("BASELINE", "1") {
                "0" => (vec![arm.clone()], arm),
                "only" => (vec!["baseline".into()], "baseline".into()),
                _ => (vec!["baseline".into(), arm.clone()], arm),
            }
        };
        for (key, value) in [
            ("SEEDS", "0 1 2"),
            ("LIMIT", "0"),
            ("RUNNER", "stub"),
            ("SKIP_DONE", "1"),
            ("ON_FAIL", "continue"),
        ] {
            self.default(key, value);
        }
        self.set("AGENT", "codex");
        let default_model = self.setting("CODEX_MODEL", "gpt-5.6-sol").to_owned();
        self.default("MODEL", &default_model);
        self.check_routing("codex");
        self.flush()?;
        if self.stopped {
            return Ok(());
        }
        self.default("DOMAIN", "swebench");
        let domain = self.get("DOMAIN").to_owned();
        if !matches!(domain.as_str(), "swebench" | "webbench") {
            self.err(format!(
                "!! DOMAIN must be swebench|webbench (got '{domain}')"
            ));
            self.stop(2);
            return Ok(());
        }
        let tasks_key = format!("TOKENECONOMIZER_{}_TASKS", domain.to_ascii_uppercase());
        let domain_tasks = self.get(&tasks_key).to_owned();
        self.default("TASKS_FILE", &domain_tasks);
        if self.get("TASKS_FILE").is_empty() {
            self.err(format!(
                "!! no task-spec JSON: set TASKS_FILE or {tasks_key}"
            ));
            self.stop(2);
            return Ok(());
        }
        let runner = self.get("RUNNER");
        if runner != "stub" && !runner.contains(&domain) {
            self.err(format!("!! RUNNER={runner} does not match DOMAIN={domain} (expected te_harness.{domain}:make_runner)"));
            self.stop(2);
            return Ok(());
        }
        let workload = self
            .setting(
                "WORKLOAD",
                &format!("{}/workloads/{domain}.json", self.cwd.display()),
            )
            .to_owned();
        if workload != "none" && !self.cwd.join(&workload).is_file() {
            self.err(format!(
                "!! workload manifest not found: {workload} (set WORKLOAD=none to run unfiltered)"
            ));
            self.stop(2);
            return Ok(());
        }
        let mut repos = self.get("REPOS").to_owned();
        if repos.is_empty() {
            if workload == "none" {
                self.err("!! WORKLOAD=none needs an explicit REPOS");
                self.stop(2);
                return Ok(());
            }
            match sweep::group_sizes(&domain, Some(workload.as_ref()), &self.cwd, &self.cwd) {
                Ok(groups) => {
                    let mut groups: Vec<_> = groups.into_keys().collect();
                    groups.sort();
                    for group in groups {
                        repos.push_str(&group);
                        repos.push(' ');
                    }
                }
                Err(error) => self.err(error.message),
            }
        }
        self.set("REPOS", repos.clone());
        let results = self
            .setting(
                "TOKENECONOMIZER_RESULTS",
                &format!("{}/results", self.cwd.display()),
            )
            .to_owned();
        self.set("RESULTS_ROOT", results.clone());
        let model = self.get("MODEL").to_owned();
        let tag = environment::model_tag(&model);
        let dest = format!(
            "{results}/{}",
            environment::tree_name("codex", &domain, &tag)
        );
        let mut cells = Vec::new();
        for seed in words(self.get("SEEDS")) {
            for repo in words(&repos) {
                for arm in &arms {
                    cells.push(format!("{arm}\t{repo}\t{seed}"));
                }
            }
        }
        if self.get("DRYRUN") == "1" {
            self.out(format!("[codex_ab] DRYRUN -- agent=codex model={model}"));
            self.out(format!("[codex_ab] dest={dest}"));
            self.out("[codex_ab] the cells this lane would run, in order:");
            for cell in cells {
                self.out(format!("  {cell}"));
            }
            self.out("[codex_ab] DRYRUN: nothing was run.");
            return Ok(());
        }
        fs::create_dir_all(self.cwd.join(&dest))?;
        self.flush()?;
        let mut detached = vec![
            std::env::current_exe()?.to_string_lossy().into_owned(),
            "codex-ab".into(),
        ];
        detached.extend_from_slice(args);
        self.detach(&format!("codex_ab__{tag}_{domain}_{lane_tag}"), &detached);
        if self.stopped {
            return Ok(());
        }
        self.fleet_cap(None);
        self.flush()?;
        if self.stopped {
            return Ok(());
        }
        self.fleet_report();
        self.out(format!("[codex_ab] domain={domain} agent=codex model={model} arms=({}) repos='{repos}' seeds='{}' runner={} dest={dest}",arms.join(" "),self.get("SEEDS"),self.get("RUNNER")));
        self.out(format!(
            "[codex_ab] cpus: per-container {}, fleet {}",
            self.get("TE_SWEBENCH_MAX_CPUS"),
            self.setting("TE_FLEET_CPUS", "16")
        ));
        self.out(format!(
            "[codex_ab] tasks={} workload={workload}",
            self.get("TASKS_FILE")
        ));
        self.out(format!(
            "[codex_ab] skip_done={} on_fail={}",
            self.get("SKIP_DONE"),
            self.get("ON_FAIL")
        ));
        self.run_cells(Workflow::Codex, &cells, &dest, &workload, &python)
    }
}

pub fn command(args: &[String], env: Map<String, Value>, cwd: PathBuf) -> io::Result<i32> {
    let mut context = Context::new(env, cwd);
    let result = context.codex_ab(args);
    context.flush()?;
    result?;
    Ok(context.code)
}
