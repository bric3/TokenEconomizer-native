//! Cell execution and resume shared by the main and Codex sweeps.
use super::Context;
use crate::experiments::{aggregate, sweep};
use std::{fs, io, path::Path, process::Command, time::Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Workflow {
    Main,
    Codex,
}
impl Workflow {
    fn name(self) -> &'static str {
        match self {
            Self::Main => "main_sweep",
            Self::Codex => "codex_ab",
        }
    }
}

impl Context {
    pub(super) fn run_cells(
        &mut self,
        workflow: Workflow,
        cells: &[String],
        dest: &str,
        workload: &str,
        python: &str,
    ) -> io::Result<()> {
        let name = workflow.name();
        let agent = self.get("AGENT").to_owned();
        let domain = self.get("DOMAIN").to_owned();
        let executable = std::env::current_exe()?;
        let workload_args: Vec<String> = if workload == "none" {
            Vec::new()
        } else {
            vec!["--workload".into(), workload.into()]
        };
        let counts = sweep::group_sizes(
            &domain,
            (workload != "none").then(|| Path::new(&workload)),
            &self.cwd,
            &self.cwd,
        );
        let counts = match counts {
            Ok(counts) => counts,
            Err(error) => {
                self.err(error.message);
                Default::default()
            }
        };
        let start = Instant::now();
        let mut failed = Vec::new();
        let mut ran = 0;
        let mut skipped = 0;
        for cell in cells {
            let mut fields = cell.split('\t');
            let arm = fields.next().unwrap();
            let repo = fields.next().unwrap();
            let seed = fields.next().unwrap();
            let mut want = counts.get(repo).copied().unwrap_or(0) as i64;
            let mut displayed_want = want.to_string();
            // shortcut: literal limits cover supported launches; audit general Bash arithmetic separately.
            if let Some(limit) = super::fleet::shell_integer(self.get("LIMIT"))
                && self.get("LIMIT") != "0"
                && want > limit
            {
                want = limit;
                displayed_want = self.get("LIMIT").to_owned();
            }
            let repo_tag = repo.rsplit('/').next().unwrap().replace('-', "_");
            let dir = self
                .cwd
                .join(dest)
                .join(arm)
                .join(format!("{repo_tag}_s{seed}"));
            let have = cell_rows(&dir);
            #[cfg(target_os = "macos")]
            let displayed = if dir.is_dir() {
                format!("{have:8}")
            } else {
                have.to_string()
            };
            #[cfg(not(target_os = "macos"))]
            let displayed = have.to_string();
            if self.get("SKIP_DONE") == "1" && want > 0 && have >= want as usize {
                self.out(format!(
                    "[{name}] SKIP  {arm} {repo} s{seed} ({displayed}/{displayed_want} rows already stored)"
                ));
                skipped += 1;
                continue;
            }
            self.out(format!(
                "[{name}] RUN   {arm} {repo} s{seed}  (have {displayed} of {displayed_want})"
            ));
            self.flush()?;
            let mut command = Command::new(&executable);
            command
                .args([
                    "--arm",
                    arm,
                    "--agent",
                    &agent,
                    "--repo",
                    repo,
                    "--seed",
                    seed,
                    "--limit",
                    self.get("LIMIT"),
                    "--adapter",
                    &domain,
                    "--tasks-file",
                    self.get("TASKS_FILE"),
                ])
                .args(&workload_args)
                .args([
                    "--results-root",
                    dest,
                    "--runner",
                    native_runner(self.get("RUNNER")),
                    "--model",
                    self.get("MODEL"),
                    "--optimizer-model",
                    if workflow == Workflow::Codex {
                        "agent"
                    } else {
                        &agent
                    },
                ])
                .current_dir(&self.cwd)
                .env_clear()
                .envs(
                    self.process_env()
                        .iter()
                        .map(|(key, value)| (key, value.as_str().unwrap())),
                )
                // The SDK's installed root can differ from the launcher's configuration root.
                .env("TOKENECONOMIZER_ENV_FILE", self.env_file());
            if workflow == Workflow::Codex {
                command.args(["--max-turns", "80"]);
            }
            command.arg("--fresh");
            if self.get("TE_BENCH_PYTHON").is_empty() {
                command.env("TE_BENCH_PYTHON", python);
            }
            let success = match command.status() {
                Ok(status) => status.success(),
                Err(error) => {
                    self.err(format!("!! could not run {arm} {repo} s{seed}: {error}"));
                    false
                }
            };
            if success {
                ran += 1;
            } else {
                self.out(if workflow == Workflow::Codex {
                    format!("[{name}] FAILED arm={arm} repo={repo} seed={seed}")
                } else {
                    format!("[{name}] FAILED {arm} {repo} s{seed}")
                });
                failed.push(format!("{arm}/{repo}/s{seed}"));
                if self.get("ON_FAIL") == "abort" {
                    self.out(format!("[{name}] ON_FAIL=abort -- stopping"));
                    break;
                }
            }
        }
        self.out(format!("[{name}] aggregating -> {dest}/index/"));
        self.flush()?;
        match aggregate::command("te-bench aggregate", &[dest.into()]) {
            Ok(output) => {
                self.stdout.push_str(&output.stdout);
                self.stderr.push_str(&output.stderr);
            }
            Err(error) => self.err(error),
        }
        self.out(format!(
            "[{name}] DONE in {} min: ran={ran} skipped={skipped} failed={}",
            start.elapsed().as_secs() / 60,
            failed.len()
        ));
        if !failed.is_empty() {
            for cell in failed {
                self.out(if workflow == Workflow::Codex {
                    format!("[{name}] FAILED CELL: {cell}")
                } else {
                    format!("[{name}]   FAILED: {cell}")
                });
            }
            if workflow == Workflow::Main {
                self.out("[main_sweep] re-run this same command to retry only the failed cells (SKIP_DONE=1");
                self.out("[main_sweep] skips the finished ones).");
            }
            self.code = 1;
        }
        if workflow == Workflow::Codex {
            self.out("[codex_ab] Compare an arm with its baseline, cell by cell:");
            self.out(format!("    te-bench compare-arms --baseline {dest}/baseline/<cell> --arm {dest}/<arm>/<cell>"));
        }
        Ok(())
    }
}

fn native_runner(spec: &str) -> &str {
    match spec {
        "te_harness.swebench:make_runner" => "te_runtime.swebench:make_runner",
        "te_harness.webbench:make_runner" => "te_runtime.webbench:make_runner",
        spec => spec,
    }
}

fn cell_rows(dir: &Path) -> usize {
    if fs::symlink_metadata(dir).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return 0;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| {
            usize::from(entry.file_name() == "row.json")
                + if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                    cell_rows(&entry.path())
                } else {
                    0
                }
        })
        .sum()
}
