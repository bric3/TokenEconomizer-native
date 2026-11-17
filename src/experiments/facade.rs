use super::{
    adapters::{Adapter, SWEBENCH_TASKS_ENV},
    spiral_stage::SpiralStage,
};
use crate::{
    config::EconomizerConfig,
    microskills::{distiller::ProposeContext, observe::Vocabulary, stage::PrefixStage},
    runner::Runner,
    toolprune::stage::{ToolingAdapter, ToolingContext, ToolingStage},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

pub type RunnerResolver<'a> = dyn FnMut(&str) -> Result<Option<Box<dyn Runner>>, String> + 'a;

pub trait DomainAdapter: ToolingAdapter {
    fn env_vocabulary(&mut self) -> Result<Vocabulary, String> {
        Ok(Vocabulary::default())
    }
    fn spiral_snapshot(&mut self, _path: &Path) -> Result<Value, String> {
        Err("adapter has no spiral_snapshot".into())
    }
}
impl DomainAdapter for Adapter {
    fn env_vocabulary(&mut self) -> Result<Vocabulary, String> {
        Ok(Adapter::env_vocabulary(self))
    }
    fn spiral_snapshot(&mut self, path: &Path) -> Result<Value, String> {
        Ok(Adapter::spiral_snapshot(self, path))
    }
}
fn default_adapter() -> Box<dyn DomainAdapter> {
    Box::new(Adapter::swebench(
        std::env::var(SWEBENCH_TASKS_ENV).ok().map(PathBuf::from),
        &Map::new(),
    ))
}

#[derive(Deserialize)]
#[serde(default)]
pub struct StreamOptions {
    pub agent: String,
    pub repo: String,
    pub results_root: PathBuf,
    pub fresh: bool,
    pub optimizer_model: String,
    pub max_turns: Option<f64>,
}
impl Default for StreamOptions {
    fn default() -> Self {
        Self {
            agent: String::new(),
            repo: String::new(),
            results_root: PathBuf::new(),
            fresh: false,
            optimizer_model: "agent".into(),
            max_turns: Some(80.0),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Economizer {
    pub config: EconomizerConfig,
    pub injection: PrefixStage,
    pub tooling: ToolingStage,
    pub spiral: SpiralStage,
    #[serde(skip, default = "default_adapter")]
    pub adapter: Box<dyn DomainAdapter>,
    #[serde(skip)]
    pub injection_runner: Option<Box<dyn Runner>>,
    #[serde(skip)]
    pub tooling_runner: Option<Box<dyn Runner>>,
    #[serde(skip)]
    pub spiral_runner: Option<Box<dyn Runner>>,
}
impl Economizer {
    pub fn new(
        config: EconomizerConfig,
        options: &StreamOptions,
        adapter: Option<Box<dyn DomainAdapter>>,
        resolve: &mut RunnerResolver<'_>,
        propose: &mut ProposeContext<'_>,
        messages: &mut Vec<String>,
    ) -> Result<Self, String> {
        let mut adapter = adapter.unwrap_or_else(default_adapter);
        let injection_runner = if config.injection.paid() {
            resolve("injection")?
        } else {
            None
        };
        // Prefix construction publishes the cadence before asking the adapter for its vocabulary.
        if config.injection.propose_every > 0 {
            propose.propose_every = config.injection.propose_every;
        }
        let injection = PrefixStage::new(
            &config.injection,
            &options.repo,
            &options.results_root,
            options.fresh,
            &options.optimizer_model,
            adapter.env_vocabulary()?,
            propose,
        )?;
        let tooling_runner = if config.tooling.judge {
            resolve("tooling")?
        } else {
            None
        };
        let mut context = ToolingContext {
            adapter: Some(adapter.as_mut()),
            ..Default::default()
        };
        let tooling = ToolingStage::new(
            config.tooling.clone(),
            &options.agent,
            &options.optimizer_model,
            Some((&options.repo, &options.results_root)),
            options.fresh,
            &mut context,
        );
        messages.extend(context.messages);
        let tooling = tooling?;
        let spiral_runner =
            if config.spiral.guard && (config.spiral.judge || config.spiral.nudge_llm) {
                resolve("spiral")?
            } else {
                None
            };
        let spiral = SpiralStage::new(
            config.spiral.clone(),
            options.max_turns,
            &options.optimizer_model,
            Some(options.agent.clone()),
        );
        Ok(Self {
            config,
            injection,
            tooling,
            spiral,
            adapter,
            injection_runner,
            tooling_runner,
            spiral_runner,
        })
    }

    pub fn from_args(
        args: &Value,
        mut options: StreamOptions,
        adapter: Option<Box<dyn DomainAdapter>>,
        resolve: &mut RunnerResolver<'_>,
        propose: &mut ProposeContext<'_>,
        messages: &mut Vec<String>,
    ) -> Result<Self, String> {
        let config = EconomizerConfig::from_args(args)?;
        options.agent = args["agent"].as_str().ok_or("expected agent")?.into();
        options.optimizer_model = args["optimizer_model"].as_str().unwrap_or("agent").into();
        options.max_turns = args.get("max_turns").map_or(Some(80.0), Value::as_f64);
        Self::new(config, &options, adapter, resolve, propose, messages)
    }

    pub fn injection_prefix(&mut self) -> String {
        if self.config.injection.enabled {
            self.injection.render()
        } else {
            String::new()
        }
    }

    pub fn build_tools(
        &mut self,
        task: &Value,
        index: i64,
        messages: &mut Vec<String>,
    ) -> Result<Option<Vec<String>>, String> {
        let mut context = ToolingContext {
            adapter: Some(self.adapter.as_mut()),
            judge: self
                .tooling_runner
                .as_mut()
                .map(|r| r.as_mut() as &mut dyn Runner),
            messages: Vec::new(),
        };
        let result = self.tooling.build_tools(task, index, &mut context);
        messages.extend(context.messages);
        result
    }

    pub fn run_kwargs(&self) -> Result<Value, String> {
        let mut out = self.tooling.run_kwargs();
        out.as_object_mut()
            .unwrap()
            .extend(self.spiral.run_kwargs()?.as_object().unwrap().clone());
        Ok(out)
    }

    pub fn after_task(
        &mut self,
        task: &Value,
        row: &Value,
        run_dir: &Path,
        srow: &mut Value,
        propose: &ProposeContext<'_>,
        messages: &mut Vec<String>,
    ) -> Result<(f64, Option<String>), String> {
        let mut context = ToolingContext {
            adapter: Some(self.adapter.as_mut()),
            judge: self
                .tooling_runner
                .as_mut()
                .map(|r| r.as_mut() as &mut dyn Runner),
            messages: Vec::new(),
        };
        let result = self.tooling.after_task(row, run_dir, srow, &mut context);
        messages.extend(context.messages);
        result?;
        let mut snapshot = |path: &Path| self.adapter.spiral_snapshot(path);
        self.spiral
            .after_task(row, run_dir, srow, Some(&mut snapshot))?;
        if self.config.injection.enabled {
            let mut context = ProposeContext {
                runner: self
                    .injection_runner
                    .as_mut()
                    .map(|r| r.as_mut() as &mut dyn Runner),
                external: None,
                propose_every: propose.propose_every,
                home: propose.home,
            };
            let (tokens, reason) = self
                .injection
                .after_task(task, run_dir, srow, &mut context)?;
            return Ok((tokens, Some(reason)));
        }
        Ok((0.0, None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        microskills::envfacts, names::repo_filename, toolprune::protected::ProtectedTools,
    };
    use serde_json::json;
    use std::{
        cell::RefCell,
        collections::BTreeSet,
        fs,
        rc::Rc,
        time::{SystemTime, UNIX_EPOCH},
    };

    struct AdapterLog {
        calls: Rc<RefCell<Vec<String>>>,
        saved: PathBuf,
    }
    impl ToolingAdapter for AdapterLog {
        fn protected_tools(&mut self) -> Result<ProtectedTools, String> {
            Ok(ProtectedTools::default())
        }
        fn tool_usage(&mut self, _: &Path) -> Result<BTreeSet<String>, String> {
            self.calls.borrow_mut().push("usage".into());
            Ok(BTreeSet::new())
        }
        fn parse_catalog_with_source(
            &mut self,
            _: &Path,
            _: &str,
        ) -> Result<(Map<String, Value>, Value), String> {
            self.calls.borrow_mut().push("catalog".into());
            Ok((
                Map::from_iter([("bash".into(), json!("shell"))]),
                json!("fixture"),
            ))
        }
        fn problem_statements(&mut self) -> Result<Map<String, Value>, String> {
            self.calls.borrow_mut().push("problems".into());
            Ok(Map::new())
        }
    }
    impl DomainAdapter for AdapterLog {
        fn env_vocabulary(&mut self) -> Result<Vocabulary, String> {
            self.calls.borrow_mut().push("vocabulary".into());
            Ok(Vocabulary::default())
        }
        fn spiral_snapshot(&mut self, _: &Path) -> Result<Value, String> {
            self.calls.borrow_mut().push("snapshot".into());
            assert!(!self.saved.exists());
            Ok(json!({}))
        }
    }

    #[test]
    fn free_injection_and_paid_guards_keep_the_stage_order() {
        let root = std::env::temp_dir().join(format!(
            "te-native-facade-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("transcript.jsonl"), "").unwrap();
        let saved = root
            .join(envfacts::STATE_SUBDIR)
            .join(format!("{}.json", repo_filename("acme/widget")));
        let calls = Rc::new(RefCell::new(Vec::new()));
        let adapter = AdapterLog {
            calls: calls.clone(),
            saved: saved.clone(),
        };
        let mut config = EconomizerConfig::default();
        config.injection.enabled = true;
        config.injection.profile = "envfacts".into();
        config.tooling.evidence = 1;
        config.tooling.judge = true;
        config.spiral.guard = true;
        config.spiral.judge = true;
        let options = StreamOptions {
            agent: "claude".into(),
            repo: "acme/widget".into(),
            results_root: root.clone(),
            fresh: true,
            ..Default::default()
        };
        let mut resolve = |stage: &str| {
            calls.borrow_mut().push(format!("resolve:{stage}"));
            Ok(None)
        };
        let mut propose = ProposeContext::default();
        let mut messages = Vec::new();
        let mut econ = Economizer::new(
            config,
            &options,
            Some(Box::new(adapter)),
            &mut resolve,
            &mut propose,
            &mut messages,
        )
        .unwrap();
        assert_eq!(
            *calls.borrow(),
            [
                "vocabulary",
                "resolve:tooling",
                "problems",
                "resolve:spiral"
            ]
        );
        let mut summary = json!({"resolved":true});
        let result = econ
            .after_task(
                &json!({"instance_id":"task"}),
                &json!({"resolved":true,"num_turns":30}),
                &root,
                &mut summary,
                &propose,
                &mut messages,
            )
            .unwrap();
        assert_eq!(result, (0.0, Some("envfacts".into())));
        assert_eq!(&calls.borrow()[4..], ["usage", "catalog", "snapshot"]);
        assert_eq!(summary["aborted"], false);
        assert!(summary["manual_facts"].is_object());
        assert!(saved.exists());
        fs::remove_dir_all(root).unwrap();
    }
}
