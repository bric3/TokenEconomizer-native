"""Pytest transport adapter: original assertions call the Rust implementation.

This is test compatibility code, retained only until the final test conversion.
Production calculations live in Rust. Original test files remain unmodified.
"""
import collections
import builtins
from collections.abc import Iterator
import contextlib
import dataclasses
import functools
import importlib
from importlib.machinery import SourceFileLoader
import inspect
import json
import math
import os
import queue
import shutil
from pathlib import Path
import subprocess
import sys
import threading
import types
import traceback

ROOT = Path(__file__).resolve().parents[1]
CALLS = collections.Counter()
LOCK = threading.Lock()
PROCESS = None
ORIGINAL_REPLACE = os.replace
LIVE_CONFIG_ENV = []
LIVE_CONFIG_FIELDS = []
NATIVE_STORE_WRITE = None
NATIVE_DENY_FUNCTIONS = {}
NATIVE_JUDGE_FUNCTIONS = {}
NATIVE_JUDGE_STORE = {}
NATIVE_QUEUE_ENQUEUE = None
NATIVE_HISTORY_FUNCTIONS = {}
NATIVE_SPAWN_FUNCTIONS = {}
NATIVE_FOLD_FUNCTIONS = {}
NATIVE_FOLD_DENY = {}
NATIVE_HOST_ANCESTORS = None
NATIVE_CONTROLLER_FUNCTIONS = {}
NATIVE_SIDECALL_FUNCTIONS = {}
NATIVE_TOOLCHECK_FUNCTIONS = {}
NATIVE_HOOK_RUN = None
NATIVE_HOOK_HOSTS = {}
NATIVE_PROBE_GIT_ENV = {}
ORIGINAL_SUBPROCESS_RUN = subprocess.run
BRIDGE_POPEN = subprocess.Popen
ORIGINAL_SOURCE_EXEC_MODULE = SourceFileLoader.exec_module
EXPERIMENT_HARNESS_REPLACEMENTS = {}


def native_cli_process(*args, **kwargs):
    argv = args[0] if args else kwargs.get("args")
    if (isinstance(argv, (list, tuple)) and len(argv) >= 2 and argv[0] == sys.executable
            and str(argv[1]) == str(ROOT / "reference/upstream/scripts/dead_code.py")):
        argv = [str(ROOT / "target/debug/te-dead-code"), "--root", str(ROOT / "reference/upstream"), *argv[2:]]
        kwargs["env"] = dict(kwargs.get("env") or os.environ, TE_PYTHON=sys.executable)
        if args: args = (argv, *args[1:])
        else: kwargs["args"] = argv
        with LOCK: CALLS["dead_code.cli_process"] += 1
    if isinstance(argv, (list, tuple)) and argv and Path(str(argv[0])).name == "bash":
        from native_launch import rewrite
        changed = rewrite(argv, kwargs)
        if list(changed) != list(argv):
            argv = changed
            if args: args = (argv, *args[1:])
            else: kwargs["args"] = argv
            with LOCK: CALLS["experiment.launch.shell_process"] += 1
    if (isinstance(argv,(list,tuple)) and len(argv) >= 2 and argv[0] == sys.executable
            and (str(argv[1]) == str(ROOT/"reference/upstream/experiments/te_harness/guards/net_guard.py")
                 or list(argv[1:3]) == ["-m","te_harness.guards.net_guard"])):
        skip = 3 if argv[1] == "-m" else 2
        argv = [str(ROOT/"target/debug"/("te-net-guard.exe" if os.name == "nt" else "te-net-guard")),*argv[skip:]]
        if args: args = (argv,*args[1:])
        else: kwargs["args"] = argv
        with LOCK: CALLS["experiment.net_guard.process"] += 1
    if (isinstance(argv, (list, tuple)) and len(argv) >= 3 and argv[0] == sys.executable
            and str(argv[1]) == str(ROOT / "reference/upstream/plugin/scripts/te_bootstrap.py")
            and argv[2] == "cli"):
        executable = "te.exe" if os.name == "nt" else "te"
        fixture = ROOT / "validation/results/opencode-sdk-tests/scripts" / executable
        program = fixture if (kwargs.get("env") or {}).get("TE_CONSOLE_PROBE") and fixture.is_file() else ROOT / "target/debug" / executable
        argv = [str(program), *argv[3:]]
        if args:
            args = (argv, *args[1:])
        else:
            kwargs["args"] = argv
        with LOCK:
            CALLS["native_cli.bootstrap_process"] += 1
    if (isinstance(argv, (list, tuple)) and argv
            and str(argv[-1]) == str(ROOT / "reference/upstream/plugin/pi/tokeneconomizer.ts")):
        fixture = ROOT / "validation/results/pi-sdk/pi/tokeneconomizer.ts"
        fixture.parent.mkdir(parents=True, exist_ok=True)
        fixture.write_text((ROOT / "src/plugin/pi/sdk.ts").read_text(encoding="utf-8"), encoding="utf-8")
        scripts = fixture.parent.parent / "scripts"
        scripts.mkdir(exist_ok=True)
        executable = "te.exe" if os.name == "nt" else "te"
        shutil.copy2(ROOT / "target/debug" / executable, scripts / executable)
        argv = [*argv[:-1], str(fixture)]
        if args:
            args = (argv, *args[1:])
        else:
            kwargs["args"] = argv
        with LOCK:
            CALLS["pi_sdk.carried_process"] += 1
    if (isinstance(argv, (list, tuple)) and len(argv) >= 2 and argv[0] == sys.executable
            and str(argv[1]) == str(ROOT / "reference/upstream/scripts/build_plugin.py")):
        argv = [str(ROOT / "target/debug" / ("te-bridge.exe" if os.name == "nt" else "te-bridge")),
                "--package-reference", *argv[2:]]
        if args:
            args = (argv, *args[1:])
        else:
            kwargs["args"] = argv
        with LOCK:
            CALLS["package.reference_process"] += 1
    if (isinstance(argv, (list, tuple)) and len(argv) >= 3 and argv[0] == sys.executable
            and list(argv[1:3]) == ["-m", "tokeneconomizer.plugin.cli"]):
        argv = [str(ROOT / "target/debug" / ("te.exe" if os.name == "nt" else "te")), *argv[3:]]
        if args:
            args = (argv, *args[1:])
        else:
            kwargs["args"] = argv
        with LOCK:
            CALLS["native_cli.process"] += 1
    return ORIGINAL_SUBPROCESS_RUN(*args, **kwargs)


def native_package_module(loader, module):
    ORIGINAL_SOURCE_EXEC_MODULE(loader, module)
    if str(loader.path) == str(ROOT / "reference/upstream/scripts/build_plugin.py"):
        module.build = lambda out: invoke("package.build_reference", {
            "source": module.REPO, "out": str(out), "droid": False})
        module.build_droid = lambda out: invoke("package.build_reference", {
            "source": module.REPO, "out": str(out), "droid": True})
    elif str(loader.path) == str(ROOT / "reference/upstream/scripts/dead_code.py"):
        from native_dead_code import install
        install(module)
    elif str(loader.path) == str(ROOT / "reference/upstream/plugin/hermes/__init__.py"):
        install_hermes_gateway(module)
    elif str(loader.path) == str(ROOT / "reference/upstream/experiments/scripts/run_experiment.py"):
        from native_experiment_providers import install
        install(driver_module=module)
    elif str(loader.path) == str(ROOT / "reference/upstream/experiments/scripts/aggregate.py"):
        from native_aggregate import install
        install(module)
    elif str(loader.path) == str(ROOT / "reference/upstream/experiments/scripts/compare_arms.py"):
        from native_compare import install
        install(module)
    elif module.__name__ == "te_bench.workloads" or str(loader.path) == str(ROOT / "reference/upstream/experiments/scripts/export_workloads.py"):
        from native_workloads import install
        install(module)
    elif str(loader.path) == str(ROOT / "reference/upstream/experiments/scripts/sweep_plan.py"):
        from native_sweep import install
        install(module)
    elif str(loader.path) == str(ROOT / "reference/upstream/experiments/scripts/export_swebench_tasks.py"):
        from native_dataset_exports import install
        install(module)
    elif str(loader.path) == str(ROOT / "reference/upstream/experiments/scripts/export_webbench_tasks.py"):
        from native_web_export import install
        install(module)
    elif str(loader.path) == str(ROOT / "reference/upstream/experiments/scripts/webbench_tasks_from_yaml.py"):
        from native_yaml_mirror import install
        install(module)
    elif module.__name__ == "te_bench.arms":
        from native_experiment_arms import install
        install(module)
    elif str(loader.path) == str(ROOT / "reference/upstream/experiments/scripts/_env.py"):
        from native_experiment_providers import install
        install(driver_module=module)
    elif module.__name__ in ("te_bench.baselines","te_bench.baselines.base","te_bench.baselines.serena","te_bench.baselines.rtk","te_bench.baselines.repomap"):
        from native_experiment_providers import install
        install(scaffold_module=module)
    elif module.__name__ == "te_harness.swebench":
        install_experiment_harness()
    elif module.__name__ == "te_harness.guards.net_guard":
        from native_experiment_net_guard import install
        install(module)
    elif module.__name__ == "te_harness.webbench":
        from native_experiment_task import install_network
        install_experiment_harness().update(install_network(module))
        from native_runner_setup import install
        EXPERIMENT_HARNESS_REPLACEMENTS.update(install(module))
        from native_experiment_providers import install
        EXPERIMENT_HARNESS_REPLACEMENTS.update(install(module))


def install_experiment_harness():
    if not EXPERIMENT_HARNESS_REPLACEMENTS:
        from native_experiment_task import install
        EXPERIMENT_HARNESS_REPLACEMENTS.update(install())
        from native_experiment_usage import install
        EXPERIMENT_HARNESS_REPLACEMENTS.update(install())
        from native_codex_install import install
        EXPERIMENT_HARNESS_REPLACEMENTS.update(install())
        from native_runner_setup import install
        EXPERIMENT_HARNESS_REPLACEMENTS.update(install())
        from native_experiment_providers import install
        EXPERIMENT_HARNESS_REPLACEMENTS.update(install())
    return EXPERIMENT_HARNESS_REPLACEMENTS


def install_hermes_gateway(module):
    original = module.Bridge
    module.__dict__.update(invoke("hermes_gateway.constants", {}))
    module.with_redirect = lambda result, text: invoke("hermes_gateway.with_redirect", {"result": result, "text": text})
    module.untrusted = lambda tool_name: invoke("hermes_gateway.untrusted", {"tool_name": tool_name})
    module.disabled = lambda: invoke("hermes_gateway.disabled", {"env": dict(os.environ)})

    class HeldLock:
        def __init__(self, bridge, sid):
            self.bridge, self.sid, self.pending = bridge, sid, None

        def acquire(self):
            self.pending = self.bridge._call("hermes_gateway.hold", {"sid": self.sid})
            return True

        def release(self):
            result = invoke("hermes_gateway.resume", {"job": self.pending["job"], "reply": {"value": None}})
            while isinstance(result, dict) and result.get("callback") == "hermes_gateway":
                result = invoke("hermes_gateway.resume", {"job": result["job"]})
            storage_result(result)
            self.pending = None

    class Bridge:
        def __init__(self, call=module.dispatch, home=module.hermes_home):
            self.call, self.home, self._records = call, home, {}
            # shortcut: handles live for this pytest process; final test conversion removes this transport.
            self._bridge = invoke("hermes_gateway.new", {})

        def _call(self, operation, fields):
            args = dict(fields, bridge=self._bridge, env=dict(os.environ), home=os.path.expanduser("~"),
                        limits={"max_ended": module.MAX_ENDED, "max_ignored": module.MAX_IGNORED,
                                "lock_timeout": module.LOCK_TIMEOUT_S})
            result = invoke(operation, args)
            exceptions = {}
            while isinstance(result, dict) and result.get("callback") == "hermes_gateway":
                method, params = result["method"], result["args"]
                if method == "locked":
                    return result
                try:
                    if method == "pending":
                        result = invoke("hermes_gateway.resume", {"job": result["job"]})
                        continue
                    if method == "home":
                        value = os.fspath(self.home())
                    elif method == "dispatch":
                        value = self.call(*params)
                    elif method == "notice":
                        value = module.logger.info("%s", params[0])
                    else:
                        raise AssertionError("Unknown Hermes callback: " + method)
                    reply = {"value": value}
                except Exception as error:
                    exceptions[str(error)] = error
                    reply = {"error": str(error)}
                result = invoke("hermes_gateway.resume", {"job": result["job"], "reply": reply})
            if isinstance(result, dict) and result.get("raised") and result.get("message") in exceptions:
                raise exceptions[result["message"]]
            return storage_result(result)

        def _record_view(self, raw):
            if raw is None:
                return None
            record = self._records.setdefault(raw["id"], {"lock": HeldLock(self, raw["id"])})
            record.update(raw)
            return record

        def session(self, session_id):
            return self._record_view(self._call("hermes_gateway.invoke", {
                "method": "session", "args": {"session_id": session_id}}))

        @property
        def sessions(self):
            raw = self._call("hermes_gateway.invoke", {"method": "snapshots", "args": {}})
            return {key: self._record_view(value) for key, value in raw.items()}

    def method(name):
        signature = inspect.signature(getattr(original, name))
        def call(self, *args, **kwargs):
            bound = signature.bind(self, *args, **kwargs)
            bound.apply_defaults()
            params = dict(bound.arguments)
            params.pop("self")
            return self._call("hermes_gateway.invoke", {"method": name, "args": params})
        return call
    for name in ("section", "pre_llm_call", "transform_tool_result", "on_session_end", "on_session_finalize"):
        setattr(Bridge, name, method(name))
    module.Bridge = Bridge


def invoke(operation, arguments):
    global PROCESS
    with LOCK:
        if PROCESS is None:
            PROCESS = subprocess.Popen(
                [str(ROOT / "target/debug/te-bridge")], stdin=subprocess.PIPE,
                stdout=subprocess.PIPE, text=True, encoding="utf-8")
        PROCESS.stdin.write(json.dumps({"op": operation, "args": arguments}) + "\n")
        PROCESS.stdin.flush()
        line = PROCESS.stdout.readline()
        if not line:
            raise RuntimeError(f"Rust bridge exited: {PROCESS.poll()}")
        response = json.loads(line)
        CALLS[operation] += 1
        if "error" in response:
            raise ValueError(response["error"])
        return response["result"]


def wrap(original, operation):
    signature = inspect.signature(original)
    generator = inspect.isgeneratorfunction(original)

    @functools.wraps(original)
    def native(*args, **kwargs):
        bound = signature.bind(*args, **kwargs)
        bound.apply_defaults()
        arguments = dict(bound.arguments)
        if operation.startswith("hook."):
            return hook_call(operation, arguments)
        if operation.startswith("cli."):
            return cli_call(operation, arguments)
        if operation.startswith("probe."):
            return probe_call(operation, arguments)
        if operation.startswith("hookqueue."):
            return hookqueue_call(operation, arguments)
        if operation.startswith("history."):
            return history_call(operation, arguments)
        if operation.startswith("fold."):
            return fold_call(operation, arguments)
        if operation.startswith("host_helpers."):
            return host_call(operation, arguments)
        if operation.startswith("host_runtime."):
            return host_session_call(operation, arguments)
        if operation.startswith("controller."):
            return controller_call(operation, arguments)
        if operation.startswith(("sidecall.", "toolcheck.")):
            return sidecall_call(operation, arguments)
        if operation.startswith("repomap."):
            return repomap_call(operation, arguments)
        if operation.startswith(("context.", "winspawn.")):
            return context_call(operation, arguments)
        if operation == "trace.copilot.copilot_mcp_servers":
            arguments["home"] = os.path.expanduser("~")
            if arguments["env"] is None:
                arguments["env"] = {"COPILOT_HOME": os.environ.get("COPILOT_HOME", "")}
        if operation in ("trace.claude.claude_projects_dir", "trace.droid.droid_sessions"):
            arguments["home"] = os.path.expanduser("~")
            if arguments["env"] is None:
                arguments["env"] = {key: os.environ.get(key, "") for key in (
                    "CLAUDE_CONFIG_DIR", "HOME", "USERPROFILE", "DROID_SESSIONS_DIR")}
        if "servers" in arguments:
            arguments["servers"] = list(arguments["servers"])
        arguments = {name: os.fspath(value) if isinstance(value, os.PathLike) else value
                     for name, value in arguments.items()}
        for name in ("lines", "turn_lines"):
            if name in arguments:
                arguments[name] = list(arguments[name])
        if operation == "config.needs_optimizer":
            arguments["config"] = dataclasses.asdict(arguments["config"])
        if operation == "live_config.load_config":
            env = os.environ if arguments["env"] is None else arguments["env"]
            arguments["env"] = {key: env[key] for key in LIVE_CONFIG_ENV if key in env}
        if operation.startswith(("paths.", "live_config.", "repos.", "applied.", "denylist.", "live_adapter.")):
            arguments["process_cwd"] = os.getcwd()
        if operation.startswith("live_adapter.") and "used_tools" in arguments:
            arguments["used_tools"] = list(arguments["used_tools"])
        if operation.startswith("denylist."):
            from tokeneconomizer.plugin import denylist as deny_module
            arguments["home"] = os.path.expanduser("~")
            if "env" in arguments:
                environment = os.environ if arguments["env"] is None else arguments["env"]
                arguments["env"] = {key: environment[key] for key in ("COPILOT_HOME", "CODEX_HOME") if key in environment}
            for name in ("tools", "items", "capabilities", "add", "remove", "raw", "catalog", "used", "servers", "tags"):
                if arguments.get(name) is not None:
                    arguments[name] = list(arguments[name])
            if operation.rsplit(".", 1)[-1] in ("apply", "restore", "set_kept", "_record", "_apply_json_list", "_restore_json_list", "_apply_codex", "_restore_codex"):
                flags = {name: getattr(deny_module, name) is not original for name, original in NATIVE_DENY_FUNCTIONS.items()}
                if arguments.get("store") is not None:
                    method = arguments["store"].write
                    flags["store_write"] = getattr(method, "__func__", method) is not NATIVE_STORE_WRITE
                arguments["transport"] = flags
        if operation.startswith("repos."):
            arguments["home"] = os.path.expanduser("~")
            if "ctx" in arguments:
                ctx = arguments.pop("ctx")
                arguments["root"], arguments["env"] = os.fspath(ctx.root), ctx.env
            if "env" in arguments:
                env = os.environ if arguments["env"] is None else arguments["env"]
                arguments["env"] = {key: env[key] for key in ("TOKENECONOMIZER_HOME", "TOKENECONOMIZER_REPO", "HOME", "GIT_CEILING_DIRECTORIES", "TE_FOLD_SYNC") if key in env}
            for name in ("repo", "current", "dst_repo"):
                if name in arguments and dataclasses.is_dataclass(arguments[name]):
                    arguments[name] = dataclasses.asdict(arguments[name])
            for name in ("st", "src", "dst"):
                if name in arguments:
                    arguments[name] = {"base": os.fspath(arguments[name].base)}
            if "cache" in arguments:
                arguments["cache"] = {key: dataclasses.asdict(value) for key, value in (arguments["cache"] or {}).items()}
            for name in ("sids", "records", "done"):
                if arguments.get(name) is not None:
                    arguments[name] = list(arguments[name])
            if operation == "repos._record_error":
                arguments["error"] = traceback.format_exc()
        if operation == "applied.write_applied":
            method = arguments["store"].write
            arguments["write_transport"] = getattr(method, "__func__", method) is not NATIVE_STORE_WRITE
            for name in ("by_path", "kept"):
                if arguments[name] is not None:
                    arguments[name] = {os.fspath(path): list(tools) for path, tools in arguments[name].items()}
        if operation.startswith("paths."):
            arguments["home"] = os.path.expanduser("~")
            if "env" in arguments:
                env = os.environ if arguments["env"] is None else arguments["env"]
                arguments["env"] = {key: env[key] for key in ("TOKENECONOMIZER_HOME", "TOKENECONOMIZER_REPO", "HOME", "GIT_CEILING_DIRECTORIES", "TE_PLUGIN_ROOT") if key in env}
            if operation == "paths.ensure_launcher":
                arguments["windows_python"] = sys.executable if os.name == "nt" else None
            if operation == "paths.repo_state_dir":
                arguments["repo"] = dataclasses.asdict(arguments["repo"])
        if operation.startswith("live_config."):
            if "value" in arguments and isinstance(arguments["value"], float) and not math.isfinite(arguments["value"]):
                arguments["nonfinite"], arguments["value"] = str(arguments["value"]), None
            if operation == "live_config._old_names_first":
                arguments["items"] = list(arguments["items"])
        obj = arguments.get("self")
        if obj is not None:
            if operation.startswith("live_adapter."):
                arguments["self"] = live_adapter_data(obj)
            elif operation.startswith("live_config."):
                arguments["self"] = vars(obj)
            elif operation == "tooling.new":
                arguments.pop("self")
                arguments["cfg"] = dataclasses.asdict(arguments["cfg"])
                arguments["runner"] = arguments.pop("judge_tool_runner")
                arguments.pop("untool_runner")
            elif operation.startswith("tooling."):
                arguments["self"] = tooling_data(obj)
                arguments["runner"], arguments["adapter"] = obj.judge_tool_runner, obj.adapter
            elif operation.startswith("toolprune.codex."):
                arguments["self"] = codex_profile_data(obj)
            elif operation.startswith(("prefix.", "layers.")):
                layers = obj.distiller if operation.startswith("prefix.") else obj
                arguments["self"] = prefix_data(obj) if operation.startswith("prefix.") else layers_data(obj)
                arguments["runner"], arguments["external"] = layers_callbacks(layers)
                arguments["environment_claims"] = getattr(arguments["external"], "__self__", None) is getattr(layers, "env", False)
                arguments["propose_every"] = importlib.import_module("tokeneconomizer.microskills.distiller").PROPOSE_EVERY
            elif operation.startswith("distiller."):
                arguments["self"] = distiller_data(obj)
                arguments["runner"] = obj.runner
                arguments["external"] = obj.external_facts
                module = importlib.import_module("tokeneconomizer.microskills.distiller")
                arguments["propose_every"] = module.PROPOSE_EVERY
            elif operation.startswith("proposed_fact."):
                arguments["self"] = proposed_fact_data(obj)
            elif operation.startswith("extractor."):
                arguments["self"] = {"spec": obj.spec}
            elif operation.startswith("detector."):
                arguments["self"] = detector_data(obj)
            elif operation.startswith("envfacts."):
                arguments["self"] = envfacts_data(obj)
            elif operation.startswith("envfact."):
                arguments["self"] = envfact_data(obj)
            elif operation.startswith("lifecycle."):
                arguments["self"] = vars(obj)
            elif operation.startswith("turn."):
                arguments["self"] = turn_data(obj)
            elif operation.startswith("state."):
                arguments["self"] = spiral_data(obj)
            else:
                arguments["self"] = vars(obj) if operation.startswith(("envelope.", "store.")) else obj.to_dict()
        if operation.startswith("store.") or operation == "lifecycle.write_json_atomic":
            from tokeneconomizer.plugin import store as store_module
            arguments["windows"] = store_module._WINDOWS if operation.startswith("store.") else os.name == "nt"
            arguments["replace_transport"] = store_module.os.replace is not ORIGINAL_REPLACE
            arguments["read_transport"] = hasattr(store_module, "open")
            if "parts" in arguments:
                arguments["parts"] = [os.fspath(part) for part in arguments["parts"]]
        if operation == "state.state_from_trajectory":
            loader = arguments.pop("load_trajectory")
            if loader is not None:
                arguments["trajectory"] = list(loader(arguments["path"]))
        if operation == "state.completed_calls":
            arguments["pend"] = {json.dumps(key, ensure_ascii=False, separators=(",", ":")): value
                                 for key, value in arguments["pend"].items()}
        if operation == "shell_edits.fold_shell_edits":
            arguments["st"] = spiral_data(arguments["st"])
        if operation == "turn.state_to_dict":
            arguments["st"] = spiral_data(arguments["st"])
        if "tracker" in arguments:
            arguments["tracker"] = vars(arguments["tracker"])
        state_store = arguments.get("store")
        if state_store is not None:
            arguments["store_transport"] = not hasattr(state_store, "base")
            arguments["store"] = {"base": state_store.base} if hasattr(state_store, "base") else {}
        if "guard" in arguments:
            arguments["guard"] = turn_data(arguments["guard"])
        state = arguments.get("state")
        if operation.startswith("rules.") and state is not None:
            arguments["state"] = {name: getattr(state, name) for name in (
                "turns", "edit_churn", "distinct_edit_files", "error_outputs", "stagnation",
                "repeated_fail_sigs", "test_runs", "edit_counts")}
        judge = arguments.get("judge")
        writer = arguments.get("writer")
        runner = arguments.get("runner")
        if operation.startswith("judge.") or operation == "pricing.runner_effective_or_approx":
            arguments["runner"] = runner is not None
        if operation == "turn.fold":
            arguments["judge"] = ("background" if hasattr(judge, "poll") else "sampled") if judge is not None else None
        if operation == "turn.nudge_body":
            arguments["writer"] = writer is not None
        if operation == "rules.assess_abort":
            arguments["judge"] = judge is not None
        if "files" in arguments:
            arguments["files"] = sorted(arguments["files"])
        if "vocab" in arguments and arguments["vocab"] is not None:
            arguments["vocab"] = vocabulary_data(arguments["vocab"])
        if operation in ("lifecycle.edited_files", "lifecycle.edited_pyfiles"):
            arguments["calls"] = list(arguments["calls"])
        if operation.startswith("observe.") and "o" in arguments:
            arguments["o"] = observation_data(arguments["o"])
        if operation == "envfacts.typed_ok":
            arguments["o"] = observation_data(arguments["o"])
        if operation == "envfacts.fold":
            arguments["fact"] = envfact_data(arguments["fact"])
        if operation == "envfacts._merged_lines":
            arguments["facts"] = [envfact_data(fact) for fact in arguments["facts"]]
        if operation == "envfacts.update" and arguments["edited"] is not None:
            arguments["edited"] = sorted(arguments["edited"])
        if operation == "envfacts.seed_injected" and arguments["ids"] is not None:
            arguments["ids"] = list(arguments["ids"])
        if operation == "envfacts._join":
            arguments["items"] = list(arguments["items"])
        if operation.startswith("stage.") and "ids" in arguments:
            arguments["ids"] = list(arguments["ids"])
        if operation == "layers.update" and arguments["edited"] is not None:
            arguments["edited"] = sorted(arguments["edited"])
        if operation == "layers.seed_injected":
            arguments["ids"] = list(arguments["ids"] or [])
        if operation.startswith(("prefix.", "layers.")):
            arguments["home"] = os.path.expanduser("~")
        if operation.startswith("distiller."):
            arguments["home"] = os.path.expanduser("~")
            for name in ("cmds", "calls", "roots", "existing_feats", "raw_excerpts", "external"):
                if name in arguments and arguments[name] is not None and not callable(arguments[name]):
                    arguments[name] = list(arguments[name])
            if "existing" in arguments and not isinstance(arguments["existing"], dict):
                arguments["existing"] = list(arguments["existing"])
            if "fact" in arguments:
                arguments["fact"] = proposed_fact_data(arguments["fact"])
        external = arguments.get("external")
        adapter = arguments.get("adapter")
        if operation.startswith("tooling."):
            arguments["adapter"] = adapter is not None
        if operation.startswith(("tooling.", "toolprune.tools.")):
            arguments["runner"] = runner is not None
        if operation.startswith("toolprune.codex."):
            for name in ("logical_capabilities", "applied_pruned", "hints", "pruned", "protected", "capabilities"):
                if name in arguments and arguments[name] is not None:
                    arguments[name] = list(arguments[name])
        if operation.startswith(("distiller.", "prefix.", "layers.")) and obj is not None:
            runner = arguments["runner"]
            arguments["runner"] = runner is not None
            arguments["external"] = external is not None
        result = invoke(operation, arguments)
        if operation.startswith("live_adapter."):
            if operation in ("live_adapter.detect_vocabulary", "live_adapter.env_vocabulary"):
                return vocabulary_from_data(result)
            if operation == "live_adapter.protected_tools":
                from tokeneconomizer.plugin.adapter import _Protected
                return _Protected(result["names"], result["patterns"])
            if operation in ("live_adapter.inapplicable_tools", "live_adapter.parse_catalog_with_source"):
                return tuple(result)
            if operation == "live_adapter.load_trajectory":
                return [tuple(row) for row in result]
            return result
        if operation.startswith("denylist."):
            while isinstance(result, dict) and result.get("callback") == "denylist":
                try:
                    if result["method"] == "store_write":
                        value = state_store.write(*result["args"])
                    elif result["method"] == "codex_config_error":
                        value = deny_module.codex_config_error(result["args"][0], cli=result["args"][1], timeout=result["args"][2])
                    else:
                        value = getattr(deny_module, result["method"])(*result["args"])
                    reply = {"value": value}
                except Exception as error:
                    reply = {"error": str(error), "error_kind": type(error).__name__}
                result = invoke("denylist.resume", {"job": result["job"], "reply": reply})
            value = storage_result(result)
            if operation == "denylist._sections":
                return [tuple(row) for row in value]
            if operation in ("denylist._toml_insert", "denylist._json_list_keys"):
                return tuple(value)
            return value
        if operation.startswith("repos."):
            from tokeneconomizer.plugin.paths import RepoId
            if operation in ("repos._resolve", "repos.plan_dir", "repos._live", "repos.migrate_dir"):
                if bound.arguments["cache"] is not None:
                    bound.arguments["cache"].clear()
                    bound.arguments["cache"].update({key: RepoId(**value) for key, value in result["cache"].items()})
                result = result["value"]
            if operation in ("repos.pinned", "repos._repo_from", "repos._resolve"):
                return RepoId(**result) if result is not None else None
            if operation == "repos._pin_store":
                from tokeneconomizer.plugin.store import Store
                store = Store.__new__(Store)
                store.base = result["base"]
                return store
            if operation == "repos.plan_dir":
                for group in result[0].values():
                    group["repo"] = RepoId(**group["repo"])
                return tuple(result)
            if operation == "repos.listing":
                return tuple(result)
            if operation in ("repos._record_moves", "repos._prune_index"):
                bound.arguments["idx"].clear()
                bound.arguments["idx"].update(result)
                return bound.arguments["idx"]
            if operation == "repos.refold_moved":
                refold_targets(result, bound.arguments["env"])
                return None
            if operation == "repos.run_migration":
                refold_targets(result["targets"], bound.arguments["env"])
                return result["done"]
            if operation == "repos.start_migration":
                if result["action"] == "inline":
                    refold_targets(result["targets"], ctx.env)
                    return True
                if result["action"] == "spawn":
                    return ctx.spawn(result["command"], result["log"])
                return False
            return result
        if operation == "paths.repo_identity":
            from tokeneconomizer.plugin.paths import RepoId
            return RepoId(**result)
        if operation.startswith("live_config."):
            if obj is not None:
                obj.__dict__.update(result["self"])
                return result["value"]
            if operation == "live_config.load_config":
                from tokeneconomizer.plugin.config import LiveConfig
                cfg = LiveConfig.__new__(LiveConfig)
                cfg.__dict__.update(result)
                return cfg
            if operation == "live_config._old_names_first":
                return [tuple(row) for row in result]
            if operation == "live_config._drop_old_names":
                bound.arguments["current"].clear()
                bound.arguments["current"].update(result)
                return None
            return result
        if operation.startswith("toolprune.codex."):
            if obj is not None:
                restore_codex_profile(obj, result["self"])
                return result["value"]
            return set(result) if operation.endswith(("._item_capabilities", "._audit_capabilities")) else result
        if operation.startswith("stage."):
            if isinstance(result, dict) and "raised" in result:
                raise KeyError(result["message"])
            if operation == "stage._saved_layers":
                return [restore_layers(None, layer) for layer in result]
            return [tuple(row) for row in result] if operation == "stage.forget_facts" else result
        if operation == "distiller._rx":
            return NativeRegex(result) if result is not None else None
        if operation in ("distiller.parse_payload",):
            return tuple(result)
        if operation in ("distiller.load_calls", "distiller.bash_cmds", "distiller._reparse_cmds"):
            return [tuple(row) for row in result]
        if operation.startswith("detector."):
            return tuple(result) if operation == "detector.verdict" else result
        if operation.startswith("extractor."):
            return result
        if operation.startswith("proposed_fact.") and obj is not None:
            restore_proposed_fact(obj, result["self"])
            return tuple(result["value"]) if operation == "proposed_fact.observe" else result["value"]
        if operation.startswith("envfact.") and obj is not None:
            restore_envfact(obj, result["self"])
            return result["value"]
        if operation.startswith("envfacts.") and obj is not None:
            restore_envfacts(obj, result["self"])
            value = result["value"]
            if isinstance(value, dict) and "raised" in value:
                raise KeyError(value["message"])
            if operation == "envfacts._select":
                return [obj.facts[fid] for fid in value]
            if operation == "envfacts.known_ids":
                return set(value)
            if operation == "envfacts.debug_row" and "_task" in value:
                restore_verdicts(value["_task"])
            return value
        if operation == "envfacts.fold":
            restore_envfact(bound.arguments["fact"], result)
            return None
        if operation == "observe.classify_command":
            return [restore_observation(value) for value in result]
        if operation == "observe.observe_task":
            result["obs"] = [restore_observation(value) for value in result["obs"]]
            return result
        if operation in ("observe._test_labels", "observe.label_rule"):
            return tuple(result) if result is not None else None
        if operation in ("lifecycle.edited_files", "lifecycle.edited_pyfiles"):
            return set(result)
        if operation.startswith(("turn.", "judge.", "store.", "distiller.", "prefix.", "layers.", "tooling.", "toolprune.tools.", "applied.")) or operation in ("pricing.runner_effective_or_approx", "lifecycle.write_json_atomic", "envelope.load_envelope"):
            replies = []
            while isinstance(result, dict) and "callback" in result:
                if result.get("guard") is not None:
                    restore_turn(obj, result["guard"])
                try:
                    if result["callback"] == "poll":
                        value = judge.poll(obj)
                    elif result["callback"] == "start":
                        value = judge.start(obj, result["digest"])
                    elif result["callback"] == "runner":
                        value = runner(result["prompt"], result["model"])
                    elif result["callback"] == "estimate":
                        value = runner.estimate_effective(result["prompt"], result["raw"])
                    elif result["callback"] == "replace":
                        Path(result["src"]).write_bytes(bytes(result["bytes"]))
                        value = store_module.os.replace(result["src"], result["dst"])
                    elif result["callback"] == "read":
                        with store_module.open(result["path"], encoding="utf-8") as source:
                            value = source.read()
                    elif result["callback"] == "store_read":
                        value = state_store.read(result["name"])
                    elif result["callback"] == "store_write":
                        value = state_store.write(result["name"], result["obj"])
                    elif result["callback"] == "external":
                        value = list(external())
                    elif result["callback"] == "adapter":
                        method = result["method"]
                        if method == "parse_catalog_with_source" and not hasattr(adapter, method):
                            value = adapter.parse_catalog(*result["args"]), "unknown"
                        else:
                            value = getattr(adapter, method)(*result["args"])
                        if method == "protected_tools":
                            value = {"names": sorted(value), "patterns": list(getattr(value, "patterns", ()))}
                        elif isinstance(value, (set, frozenset)):
                            value = sorted(value)
                    else:
                        value = (writer if result["callback"] == "writer" else judge)(result["digest"])
                    replies.append({"value": value, "guard": turn_data(obj) if operation.startswith("turn.") and obj is not None else None})
                except Exception as error:
                    replies.append({"error": str(error), "error_kind": type(error).__name__})
                result = invoke(operation, dict(arguments, replies=replies))
        if operation == "toolprune.tools.judge_tools_for_task":
            return tuple(result)
        if operation.startswith("tooling."):
            if operation == "tooling.new":
                obj.adapter, obj.judge_tool_runner = adapter, runner
            restore_tooling(obj, result["self"])
            for message in result["messages"]:
                print(message, flush=True)
            if operation == "tooling.after_task":
                srow = bound.arguments["srow"]
                srow.clear()
                srow.update(result["srow"])
            return result.get("value")
        if operation.startswith(("prefix.", "layers.")):
            if operation.startswith("prefix."):
                restore_prefix(obj, result["self"])
                if operation == "prefix.after_task":
                    srow = bound.arguments["srow"]
                    srow.clear()
                    srow.update(result["srow"])
                    restore_manual_verdicts(srow["manual_facts"])
                    return tuple(result["value"])
            else:
                restore_layers(obj, result["self"])
                if operation == "layers.debug_row":
                    restore_manual_verdicts(result["value"])
            return result["value"]
        if operation.startswith("distiller.") and obj is not None:
            restore_distiller(obj, result["self"])
            value = result["value"]
            if isinstance(value, dict) and "raised" in value:
                raise KeyError(value["message"])
            if operation == "distiller._admit":
                bound.arguments["spec"].clear()
                bound.arguments["spec"].update(result["spec"])
                return tuple(value)
            if operation == "distiller._featurize":
                bound.arguments["dg"].clear()
                bound.arguments["dg"].update(value)
                return bound.arguments["dg"]
            if operation in ("distiller._confirms", "distiller.known_ids"):
                return set(value)
            if operation == "distiller._build_fact":
                from tokeneconomizer.microskills.distiller import Fact
                fact = Fact.__new__(Fact)
                restore_proposed_fact(fact, value)
                return fact
            if operation == "distiller._observe_fact":
                restore_proposed_fact(bound.arguments["fact"], value["fact"])
                return tuple(value["value"])
            return value
        if operation.startswith("store.") or operation in ("lifecycle.write_json_atomic", "applied.write_applied"):
            return storage_result(result)
        if operation in ("envelope.load_envelope", "envelope.save_envelope", "envelope.record_turn"):
            result = storage_result(result)
            if operation == "envelope.save_envelope":
                return result
            from tokeneconomizer.loopguard.envelope import EnvelopeTracker
            tracker = EnvelopeTracker.__new__(EnvelopeTracker)
            tracker.__dict__.update(result)
            return tracker
        if operation.startswith("turn.") and obj is not None:
            restore_turn(obj, result["guard"], replace_states=operation == "turn.after_nudge")
            value = result["value"]
            return tuple(value) if operation == "turn.fold" and value is not None else value
        if operation == "shell_edits.fold_shell_edits":
            restore_spiral(bound.arguments["st"], result)
            return None
        if operation in ("shell_edits._split_heredocs", "turn._pieces"):
            return tuple(result)
        if operation == "state.update":
            restore_spiral(obj, result)
            return None
        if operation in ("state.state_from_trajectory", "turn.state_from_dict"):
            from tokeneconomizer.loopguard.state import SpiralState
            state = SpiralState.__new__(SpiralState)
            restore_spiral(state, result)
            return state
        if operation == "state.completed_calls":
            bound.arguments["pend"].clear()
            bound.arguments["pend"].update({json.loads(key): value for key, value in result["pend"].items()})
            return [tuple(call) for call in result["calls"]]
        if operation == "rules.assess_abort" and isinstance(result, dict):
            try:
                verdict = judge(state.recent_digest(envelope=arguments["envelope"]))
            except Exception:
                verdict = None
            result = invoke(operation, dict(arguments, judge_result=verdict))
        if operation in ("lifecycle.update", "envelope.update"):
            obj.__dict__.update(result)
            return None
        if operation in ("accounting.effective", "rules.assess_abort"):
            return tuple(result)
        if operation in ("trace.claude.claude_calls", "trace.codex.codex_exec_calls",
                         "trace.copilot.copilot_event_calls", "trace.calls.load_trajectory"):
            return [tuple(call) for call in result]
        if operation == "trace.copilot.copilot_turns":
            return [[tuple(call) for call in turn] for turn in result]
        if operation in ("trace.catalog.parse_catalog_with_source", "trace.catalog._catalog_rank"):
            return tuple(result)
        if operation in ("trace.pi.pi_json_reply", "trace.opencode.opencode_run_reply", "trace.opencode._load"):
            return tuple(result)
        if operation in ("trace.hermes.hermes_tool", "trace.hermes.hermes_result", "trace.hermes._load",
                         "trace.hermes.hermes_session_row"):
            return tuple(result)
        if operation in ("trace.antigravity.antigravity_tool", "trace.antigravity.antigravity_result",
                         "trace.antigravity.antigravity_read"):
            return tuple(result)
        if operation == "trace.antigravity._read_rows":
            return [tuple(row) for row in result]
        if operation in ("trace.grok.grok_tool", "trace.grok.grok_json_reply", "trace.grok.grok_result"):
            return tuple(result) if result is not None else None
        if operation == "trace.catalog.catalog_sources":
            return [tuple(source) for source in result]
        if operation == "trace.claude.claude_history_logs":
            return [tuple(row) for row in result]
        if operation == "trace.codex._shell_patch_paths":
            return tuple(result)
        if operation == "trace.copilot.copilot_mcp_servers":
            return set(result)
        if operation == "trace.claude.claude_deferred_tools":
            return set(result)
        if generator:
            return iter(result)
        return result

    return native


def pytest_configure(config):
    global NATIVE_PROBE_GIT_ENV
    NATIVE_PROBE_GIT_ENV = {key: os.environ[key] for key in ("HOME", "USERPROFILE", "GNUPGHOME")
                            if key in os.environ}
    subprocess.run = native_cli_process
    SourceFileLoader.exec_module = native_package_module
    sys.path.insert(0, str(ROOT / "reference/upstream"))
    groups = {
        "pricing": ("resolve_model", "weights_for", "has_breakdown", "cache_write_1h_tokens",
                    "effective_tokens", "approx_tokens", "runner_effective_or_approx"),
        "ledger": ("overhead", "effective_exec"),
        "plugin.accounting": ("effective", "overhead_total", "summarize", "fmt_tokens"),
        "plugin.sidecall": ("_obj", "_env", "_hermes_env", "_pause_path", "paused", "resume", "_log", "_no_reply", "claude_cli", "hermes_cli", "_shim_argv", "program", "claude_json", "copilot_model", "droid_enabled", "_droid_usage", "local_routes", "call_name", "_claude_help", "claude_flags", "claude_no_mcp", "claude_call", "codex_call", "pi_extension_args", "pi_call", "cursor_call", "grok_call", "opencode_flags", "opencode_call", "copilot_call", "_droid_sessions_root", "_droid_settings_usage", "droid_call", "fm_available", "fm_call", "hermes_home", "_hermes_db", "_hermes_ledger", "_hermes_charge", "_hermes_delete", "_hermes_settle", "_hermes_defer", "_hermes_settle_earlier", "hermes_call", "_noted", "_host_call", "local_runner", "runner_for"),
        "plugin.toolcheck": ("_request_tools", "_prompt_tokens", "_codex_prompt_chars", "_copilot_version", "verify_codex", "verify_copilot", "measure", "copilot_capture", "_init_tools", "_kill_tree", "refresh_catalog"),
        "plugin.repomap": ("_repo_dir", "_state_path", "_cache_dir", "read_state", "_write_state", "head_commit", "_has_aider", "_aider_command_python", "find_python", "_gen_env", "generate", "refresh", "stale", "spawn_refresh", "cached_block", "prompt_idents", "for_prompt"),
        "plugin.hook": ("_truthy", "run", "_record_error", "main"),
        "plugin.cli": ("_split_argline", "_expand_argline", "_unwritable", "_refuse_unwritable", "_config_view", "_status_explanations", "_session_note", "_proposal", "cmd_status", "cmd_config", "cmd_microskills", "_microskills_forget", "_short", "_session_ctx", "cmd_fold", "cmd_judge", "_when", "_print_history", "cmd_history", "cmd_repomap", "cmd_repos", "_print_moves", "cmd_toolprune", "_toolprune_show", "_wait_and_apply", "cmd_doctor", "cmd_probe", "_recent_host", "_rtk_hook", "build_parser", "main"),
        "plugin.probe": ("test_commands", "_write", "make_repo", "_plugin_root", "_session_env", "run_session", "inspect_session", "_wait_folded", "_line", "run"),
        "plugin.config": ("canonical", "_threshold_name", "_read", "_old_names_first", "load_config", "_drop_old_names", "write_config", "unset_config", "_write"),
        "plugin.adapter": ("detect_vocabulary", "catalog_path", "load_catalog"),
        "plugin.hookqueue": ("_safe", "_queues", "enqueue", "drain", "_replay", "_prune", "run"),
        "plugin.history": ("_record_error", "read_state", "_num", "_stale", "_ledger_size", "_session_records", "_nothing_folded", "_live_folds", "_folded_by_import", "_refusal", "_identity", "is_this_repository", "_headless", "plan", "_still_eligible", "replay_envelope", "_update", "_save", "_say", "run", "_run_locked", "_import_one", "maybe_start", "notice", "summary", "describe"),
        "plugin.fold": ("_run_dir", "_microskills_config", "_fold_microskills", "forget_microskills", "enough_evidence", "_fold_toolprune", "_server_summary", "_fold_toolprune_codex", "unprotected", "_hold", "last_activity", "apply_pending", "schedule_pending", "import_blocks", "fold_session", "fold_locked", "pending_sessions", "spawn_fold"),
        "plugin.hosts": ("claude_success_code", "response_text", "merge_outputs", "_failure_response", "_exit_code", "default_host", "detect", "copilot_home", "_ancestor_args"),
        "plugin.winspawn": ("_windowless_python", "spawn", "_spawn_wmi", "_run"),
        "plugin.paths": ("state_root", "secure_root", "launcher_path", "ensure_launcher", "home_dir", "_gitdir_of_file", "is_git_entry", "find_repo_root", "_git_common_dir", "origin_url", "normalize_remote", "_slug", "digest", "_main_checkout", "repo_identity", "repo_state_dir"),
        "plugin.repos": ("repos_dir", "_repo_dir", "_record_error", "_pin_store", "_pin_name", "_repo_from", "pinned", "pin", "prune_pins", "_legacy_root", "legacy_key", "_host_dirs", "_sessions", "_transcript_cwd", "session_cwd", "_imported", "_transcript_of", "_has_learned", "dir_label", "note_repo", "_write_lines", "_rename_label", "_take_learned", "_merge_applied", "_merge_envelope", "_merge_host", "merge_whole", "_resolve", "plan_dir", "_live", "migrate_dir", "_record_moves", "_candidates", "_prune_index", "adopt", "migrate_all", "merge", "needs_migration", "start_migration", "run_migration", "migration_notice", "refold_moved", "settle", "how_words", "describe", "ignored_git_entries", "listing", "home_git_note"),
        "plugin.denylist": ("_canon", "applied_by_path", "kept_by_path", "write_applied", "supports", "unit", "settings_path", "_read_settings", "_guard_target", "write_json", "_write_text", "_exclude_from_git", "kept_for", "set_kept", "applied", "applied_for", "_record", "_json_list_keys", "_apply_json_list", "_restore_json_list", "_sections", "_key_set", "_toml_insert", "_toml_remove", "_toml_valid", "_git_tracked", "codex_config_error", "_apply_codex", "_restore_codex", "copilot_trusted_folders", "copilot_folder_trusted", "codex_home", "codex_project_trusted", "apply", "restore", "copilot_server_proposal"),
        "microskills.lifecycle": ("write_json_atomic", "edited_files", "edited_pyfiles"),
        "microskills.observe": ("unwrap", "segments", "_tokens", "repository_top", "_cd_target", "_shell_path", "_shell_isabs",
                                "classify_command", "_runner_script", "_probe_kind", "error_class", "_exit_code_hint", "_label_failed",
                                "_test_labels", "label_rule", "_env_calls", "form_failed", "form_worked", "observe_task", "_error_line",
                                "_base", "_not_found_name", "is_bare"),
        "microskills.envfacts": ("fold", "typed_ok", "_repairs", "_tool_of", "_moves_shell", "_usage", "_root", "_join", "lint_ok"),
        "microskills.distiller": ("load_calls", "bash_cmds", "build_digest", "build_generic_digest", "_command_failed", "_relative", "error_signature",
                                  "_templates", "proposer_view", "_rx", "build_proposer_prompt", "parse_payload", "_detector_fields_ok", "_reparse_cmds", "_forgotten_evidence"),
        "microskills.stage": ("_saved_layers", "forget_facts", "forgotten_facts"),
        "toolprune.codex": ("_item_capabilities", "task_hints", "_wire_text", "_audit_capabilities", "inspect_usage", "relay_capabilities", "_contract_fingerprint"),
        "toolprune.tools": ("judge_tools_for_task",),
        "names": ("repo_filename",),
        "config": ("needs_optimizer",),
        "loopguard.rules": ("scaled_thresholds", "gate_for_judge", "is_stuck", "nudge_text", "assess_abort"),
        "loopguard.judge": ("llm_stuck_judge", "llm_nudge_writer"),
        "loopguard.state": ("canon_tool", "basename", "_failure_sig", "completed_calls", "state_from_trajectory"),
        "loopguard.shell_edits": ("shell_edit_targets", "shell_revert_targets", "fold_shell_edits", "_split_heredocs"),
        "loopguard.turn": ("exit_status_covers", "status_of_tests", "runner_verdict", "_pieces", "state_to_dict", "state_from_dict"),
        "loopguard.envelope": ("_percentile", "thresholds", "base_thresholds", "load_envelope", "save_envelope", "record_turn"),
        "trace.jsonl": ("lines", "records"),
        "trace.hermes": ("hermes_tool", "_json_object", "hermes_result", "_load", "_prompts", "_tool_calls",
                         "_int", "_kept", "_private", "hermes_session_info", "hermes_prompt_count",
                         "hermes_session_to_claude", "hermes_row_usage", "hermes_session_usage", "hermes_export",
                         "hermes_session_row", "hermes_find_session"),
        "trace.droid": ("claude_tool_name", "droid_records", "_epoch", "droid_session_header", "droid_session_cwd",
                        "droid_log_head", "droid_safe_session_id", "_droid_is_prompt", "_droid_content", "_claude_line",
                        "droid_prompt_count", "droid_prompt_turns", "droid_settings_path", "droid_session_to_claude",
                        "droid_session_usage", "droid_sessions"),
        "trace.grok": ("grok_tool", "_bytes_text", "grok_result", "_content_text", "grok_session_to_claude",
                       "grok_prompt_count", "grok_session_usage", "grok_json_reply"),
        "trace.cursor": ("_response_text", "_exit_code", "cursor_events_to_claude", "cursor_prompt_count"),
        "trace.antigravity": ("antigravity_tool", "antigravity_result", "_prompt_text", "_read_rows", "_is_result",
                              "antigravity_read", "antigravity_session_to_claude", "antigravity_session_usage",
                              "antigravity_prompt_count"),
        "trace.pi": ("pi_session_header", "pi_active_branch", "pi_prompt_count", "_pi_usage", "_pi_text",
                     "pi_session_to_claude", "pi_json_reply", "pi_session_usage"),
        "trace.opencode": ("_nested", "_load", "_text", "_usage", "opencode_session_info", "opencode_prompt_count",
                           "opencode_session_to_claude", "opencode_session_usage", "opencode_run_reply"),
        "trace.catalog": ("parse_tool_catalog_flat", "parse_tool_catalog", "_load_claude_catalog_snapshot",
                          "parse_tool_catalog_claude", "parse_tool_names_copilot", "catalog_sources",
                          "parse_catalog_with_source", "parse_catalog", "_catalog_rank"),
        "trace.calls": ("strip_rtk_prefix", "_is_codex_exec", "load_trajectory", "call_cwds", "tool_usage"),
        "trace.codex": ("_shell_patch_paths", "_command_text", "codex_exec_calls", "rollout_item_to_exec",
                        "codex_rollout_to_exec", "codex_capability_trace", "codex_prompt_count",
                        "codex_session_meta", "codex_session_usage"),
        "trace.copilot": ("copilot_event_calls", "copilot_session_cwd", "copilot_session_meta",
                          "copilot_session_usage", "copilot_turns", "copilot_prompt_count",
                          "copilot_mcp_servers", "mcp_server_of"),
        "trace.claude": ("claude_calls", "_is_prompt", "_is_notification",
                         "_epoch", "claude_projects_dir", "claude_history_logs", "claude_log_head", "claude_tool_usage",
                         "claude_safe_session_id", "claude_project_folder", "claude_prompt_count",
                         "claude_prompt_turns", "claude_turn_interrupted", "claude_turn_answered",
                         "claude_subagent_files", "claude_session_usage", "claude_deferred_tools"),
    }
    replacements = {}
    for module_name, functions in groups.items():
        module = importlib.import_module("tokeneconomizer." + module_name)
        for name in functions:
            original = getattr(module, name)
            operation = module_name.removeprefix("plugin.") + "." + name
            if module_name == "plugin.config":
                operation = "live_config." + name
            if module_name == "plugin.adapter":
                operation = "live_adapter." + name
            if module_name == "plugin.hookqueue":
                operation = "hookqueue." + name
            if module_name == "plugin.history":
                operation = "history." + name
            if module_name == "plugin.hosts":
                operation = ("host_runtime." if name in ("copilot_home", "_ancestor_args") else "host_helpers.") + name
            if module_name == "plugin.winspawn":
                operation = "winspawn." + name
            if module_name == "plugin.denylist":
                operation = ("applied." if name in ("_canon", "applied_by_path", "kept_by_path", "write_applied") else "denylist.") + name
            operation = operation.removeprefix("loopguard.")
            operation = operation.removeprefix("microskills.")
            replacement = wrap(original, operation)
            replacements[original] = replacement
            setattr(module, name, replacement)

    from tokeneconomizer.plugin.config import LiveConfig
    defaults = invoke("live_config.new", {"fields": {}})
    assert defaults == vars(LiveConfig())
    global LIVE_CONFIG_FIELDS
    LIVE_CONFIG_FIELDS = tuple(defaults)
    global LIVE_CONFIG_ENV
    LIVE_CONFIG_ENV = invoke("live_config.environment", {})
    from tokeneconomizer.plugin import repos, hosts, sidecall
    constants = invoke("sidecall.constants", {})
    for name in ("TIMEOUT_S", "FM_CLI", "FM_TIMEOUT_S", "GROK_TOOLS", "GROK_REMOVED", "OPENCODE_CONFIG", "DROID_TOOLS", "HERMES_SOURCE", "HERMES_DELETE_TIMEOUT_S", "HERMES_TITLE_WAIT_S", "HERMES_ABANDON_S"):
        assert constants[name] == getattr(sidecall, name)
    for name in ("PI_FLAGS", "GROK_DENY_RULES", "COPILOT_TOOLS", "HERMES_FLAGS"):
        assert tuple(constants[name]) == getattr(sidecall, name)
    assert set(constants["NO_SIDE_CALLS"]) == sidecall.NO_SIDE_CALLS
    assert constants["JUDGE_SCHEMA"] == sidecall.JUDGE_SCHEMA
    assert constants["ROLE_SCHEMAS"] == sidecall.ROLE_SCHEMAS
    from tokeneconomizer.plugin import repomap
    repomap.GEN = str(ROOT / "validation/native_repomap_gen.py")
    constants = invoke("repomap.constants", {})
    for name, value in constants.items():
        assert value == getattr(repomap, name)
    global NATIVE_SIDECALL_FUNCTIONS
    NATIVE_SIDECALL_FUNCTIONS = {name: getattr(sidecall, name) for name in ("claude_cli", "hermes_cli", "program", "claude_call", "codex_call", "copilot_call", "pi_call", "opencode_call", "cursor_call", "grok_call", "hermes_call", "droid_call", "fm_call", "fm_available")}
    NATIVE_SIDECALL_FUNCTIONS["effective"] = sidecall.accounting.effective
    NATIVE_SIDECALL_FUNCTIONS["temporary_directory"] = sidecall.tempfile.TemporaryDirectory
    NATIVE_SIDECALL_FUNCTIONS["access"] = sidecall.os.access
    constants = invoke("repos.constants", {})
    assert set(constants["HOSTS"]) == set(hosts.HOSTS)
    assert tuple(constants["LEARNED"]) == repos.LEARNED
    assert constants["LAYOUT"] == repos.LAYOUT
    LiveConfig.__init__ = live_config_constructor(LiveConfig.__init__)
    for name in ("as_dict", "set", "_set_threshold"):
        setattr(LiveConfig, name, wrap(getattr(LiveConfig, name), "live_config." + name))

    from tokeneconomizer.microskills.lifecycle import Lifecycle
    Lifecycle.__init__ = state_constructor(Lifecycle, "lifecycle.new")
    Lifecycle.from_dict = classmethod(lifecycle_from_record)
    for name in ("_drifted_confirms", "score", "firing_rate", "injectable", "update", "to_dict"):
        setattr(Lifecycle, name, wrap(getattr(Lifecycle, name), "lifecycle." + name))

    from tokeneconomizer.microskills.envfacts import EnvFact, EnvFactsDistiller
    EnvFact.__init__ = envfact_constructor
    EnvFact.from_dict = classmethod(envfact_from_record)
    for name in ("claim", "_best", "working_form", "to_dict", "injectable", "firing_rate"):
        setattr(EnvFact, name, wrap(getattr(EnvFact, name), "envfact." + name))
    EnvFact.state = property(lambda self: invoke("envfact.state", {"self": envfact_data(self)})["value"])
    EnvFactsDistiller.__init__ = envfacts_constructor
    EnvFactsDistiller.load = classmethod(envfacts_load)
    EnvFactsDistiller._path = staticmethod(wrap(EnvFactsDistiller._path, "envfacts._path"))
    for name in ("render", "injected_ids", "seed_injected", "injectable_claims", "_select", "_merged_lines", "update", "save", "debug_row", "known_ids", "forget", "unforget", "drop_forgotten"):
        setattr(EnvFactsDistiller, name, wrap(getattr(EnvFactsDistiller, name), "envfacts." + name))
    EnvFactsDistiller.header = property(lambda self: invoke("envfacts.header", {"self": envfacts_data(self)})["value"])

    from tokeneconomizer.microskills import distiller as proposed
    proposed.Detector.__init__ = detector_constructor
    proposed.DetectorV3Scope.__init__ = detector_v3_constructor
    for cls, names in ((proposed.Detector, ("_exercised", "_fired", "verdict")),
                       (proposed.DetectorV3Scope, ("_lookup", "_exercised", "_fired"))):
        for name in names:
            setattr(cls, name, wrap(getattr(cls, name), "detector." + name))
    proposed.Extractor.__init__ = extractor_constructor
    for name in ("compute", "to_dict"):
        setattr(proposed.Extractor, name, wrap(getattr(proposed.Extractor, name), "extractor." + name))
    proposed.Fact.__init__ = proposed_fact_constructor
    proposed.Fact.from_dict = classmethod(proposed_fact_from_record)
    for name in ("observe", "injectable", "firing_rate", "to_dict"):
        setattr(proposed.Fact, name, wrap(getattr(proposed.Fact, name), "proposed_fact." + name))
    proposed.Fact.state = property(lambda self: invoke("proposed_fact.state", {"self": proposed_fact_data(self)})["value"])
    proposed.Distiller.__init__ = distiller_constructor
    proposed.Distiller.load = classmethod(distiller_load)
    proposed.Distiller._path = staticmethod(wrap(proposed.Distiller._path, "distiller._path"))
    for name in ("_observe_fact", "_featurize", "_sensed", "_coverage", "_abstaining_excerpts", "_maybe_propose", "_admit_extractor",
                 "_confirms", "_evidence", "_restates_forgotten", "_ground", "_build_fact", "_frozen_view", "_admit", "render", "update",
                 "save", "debug_row", "known_ids", "forget", "unforget", "drop_forgotten"):
        setattr(proposed.Distiller, name, wrap(getattr(proposed.Distiller, name), "distiller." + name))
    prompts = invoke("distiller.prompts", {})
    proposed.PROPOSER_SYSTEM = prompts["paper"]
    proposed.PROPOSER_SYSTEM_GENERIC = prompts["generic"]

    from tokeneconomizer.microskills.stage import PrefixStage, ComposedDistiller
    PrefixStage.__init__ = prefix_constructor
    for name in ("render", "injected_ids", "after_task"):
        setattr(PrefixStage, name, wrap(getattr(PrefixStage, name), "prefix." + name))
    for name in ("render", "injected_ids", "seed_injected", "update", "save", "debug_row"):
        setattr(ComposedDistiller, name, wrap(getattr(ComposedDistiller, name), "layers." + name))
    ComposedDistiller.opt_tokens = property(lambda self: invoke("layers.opt_tokens", {"self": layers_data(self)})["value"])
    ComposedDistiller.now = property(lambda self: invoke("layers.now", {"self": layers_data(self)})["value"])

    from tokeneconomizer.toolprune import codex as capabilities, tools as toolsets
    from tokeneconomizer.toolprune.stage import ToolingStage
    constants = invoke("toolprune.tools.constants", {})
    assert frozenset(constants["PROTECTED_TOOLS"]) == toolsets.PROTECTED_TOOLS
    assert tuple(constants["INAPPLICABLE_TOOLS"]) == toolsets.INAPPLICABLE_TOOLS
    assert constants["TOOL_JUDGE_DOMAIN"] == toolsets.TOOL_JUDGE_DOMAIN
    assert constants["CONTRACT_FINGERPRINT"] == capabilities.CONTRACT_FINGERPRINT
    toolsets.PROTECTED_TOOLS = frozenset(constants["PROTECTED_TOOLS"])
    toolsets.INAPPLICABLE_TOOLS = tuple(constants["INAPPLICABLE_TOOLS"])
    toolsets.TOOL_JUDGE_DOMAIN = constants["TOOL_JUDGE_DOMAIN"]
    capabilities.CONTRACT_FINGERPRINT = constants["CONTRACT_FINGERPRINT"]
    capabilities.CodexUntoolProfile.__init__ = codex_profile_constructor
    for name in ("_new_state", "_load", "save", "_choose_probe", "_eligible_demotions", "begin_task", "_transition", "observe"):
        setattr(capabilities.CodexUntoolProfile, name, wrap(getattr(capabilities.CodexUntoolProfile, name), "toolprune.codex." + name))
    ToolingStage.__init__ = wrap(ToolingStage.__init__, "tooling.new")
    for name in ("build_tools", "run_kwargs", "after_task"):
        setattr(ToolingStage, name, wrap(getattr(ToolingStage, name), "tooling." + name))

    from tokeneconomizer.loopguard.state import SpiralState
    SpiralState.__init__ = lambda self: restore_spiral(self, invoke("state.new", {}))
    for name in ("update", "recent_digest", "snapshot"):
        setattr(SpiralState, name, wrap(getattr(SpiralState, name), "state." + name))
    SpiralState.edit_churn = property(lambda self: invoke("state.edit_churn", {"self": spiral_data(self)}))
    SpiralState.distinct_edit_files = property(lambda self: invoke("state.distinct_edit_files", {"self": spiral_data(self)}))
    SpiralState.stagnation = property(lambda self: invoke("state.stagnation", {"self": spiral_data(self)}))

    from tokeneconomizer.loopguard.turn import TurnGuard
    TurnGuard.__init__ = turn_constructor
    for name in ("to_dict", "resolved_proxy", "verification_state", "live_digest", "summary", "after_nudge", "nudge_body", "fold"):
        setattr(TurnGuard, name, wrap(getattr(TurnGuard, name), "turn." + name))
    pattern = NativeVerifyPattern()
    importlib.import_module("tokeneconomizer.loopguard").VERIFY_RE = pattern
    importlib.import_module("tokeneconomizer.loopguard.turn").VERIFY_RE = pattern

    from tokeneconomizer import config as mechanism_config
    for cls in (mechanism_config.InjectionConfig, mechanism_config.ToolingConfig,
                mechanism_config.SpiralConfig):
        cls.__init__ = config_constructor(cls)
    mechanism_config.InjectionConfig.paid = property(
        lambda self: invoke("config.injection_paid", vars(self)))
    mechanism_config.ToolingConfig.enabled = property(
        lambda self: invoke("config.tooling_enabled", vars(self)))
    mechanism_config.SpiralConfig.enabled = property(
        lambda self: invoke("config.spiral_enabled", vars(self)))
    mechanism_config.EconomizerConfig.from_args = classmethod(config_from_args)

    from tokeneconomizer.plugin.store import Store
    Store.__init__ = store_constructor
    for name in ("path", "ensure", "read", "write", "append", "read_lines"):
        setattr(Store, name, wrap(getattr(Store, name), "store." + name))
    Store.locked = store_locked
    global NATIVE_STORE_WRITE
    NATIVE_STORE_WRITE = Store.write
    from tokeneconomizer.plugin import denylist
    global NATIVE_DENY_FUNCTIONS
    NATIVE_DENY_FUNCTIONS = {name: getattr(denylist, name) for name in ("write_json", "_write_text", "_git_tracked", "codex_config_error")}

    from tokeneconomizer.plugin import judge as live_judge
    from tokeneconomizer.loopguard import judge as loop_judge
    global NATIVE_JUDGE_FUNCTIONS, NATIVE_JUDGE_STORE
    NATIVE_JUDGE_FUNCTIONS = {name: getattr(loop_judge, name) for name in ("llm_stuck_judge", "llm_nudge_writer")}
    NATIVE_JUDGE_STORE = {name: getattr(Store, name) for name in ("read", "write", "append")}
    live_judge.AsyncJudge.__init__ = live_judge_constructor
    live_judge.AsyncJudge.start = live_judge_start
    live_judge.AsyncJudge.poll = live_judge_poll
    live_judge.NoVerdict.start = no_verdict_start
    live_judge.NoVerdict.poll = no_verdict_poll
    replacements[live_judge.run_job] = live_judge_run_job
    replacements[live_judge.spawn_job] = live_judge_spawn_job
    live_judge.run_job = live_judge_run_job
    live_judge.spawn_job = live_judge_spawn_job

    from tokeneconomizer.plugin import adapter as live_adapter
    constants = invoke("live_adapter.constants", {})
    for name in ("INTERACTIVE_PROTECTED", "COPILOT_PROTECTED", "HIGH_RISK"):
        assert frozenset(constants[name]) == getattr(live_adapter, name)
        setattr(live_adapter, name, frozenset(constants[name]))
    assert constants["HIGH_RISK_FACTOR"] == live_adapter.HIGH_RISK_FACTOR
    live_adapter.HIGH_RISK_FACTOR = constants["HIGH_RISK_FACTOR"]
    live_adapter.LiveAdapter.__init__ = live_adapter_constructor
    live_adapter._Protected.__contains__ = protected_contains
    for name in ("load_trajectory", "tool_usage", "parse_catalog", "parse_catalog_with_source", "protected_tools", "inapplicable_tools", "hardcoded_exclusions", "spiral_snapshot", "problem_statements", "env_vocabulary"):
        setattr(live_adapter.LiveAdapter, name, wrap(getattr(live_adapter.LiveAdapter, name), "live_adapter." + name))

    from tokeneconomizer.plugin import hookqueue
    constants = invoke("hookqueue.constants", {})
    assert constants["WAITS"] == hookqueue.WAITS
    assert frozenset(constants["QUEUED"]) == hookqueue.QUEUED
    hookqueue.WAITS = constants["WAITS"]
    hookqueue.QUEUED = frozenset(constants["QUEUED"])
    for name in ("DRAIN_MAX_EVENTS", "DRAIN_BUDGET_S", "IDLE_S"):
        assert constants[name] == getattr(hookqueue, name)
        setattr(hookqueue, name, constants[name])
    global NATIVE_QUEUE_ENQUEUE
    NATIVE_QUEUE_ENQUEUE = hookqueue.enqueue

    from tokeneconomizer.plugin import history
    constants = invoke("history.constants", {})
    for name, value in constants.items():
        original = getattr(history, name)
        assert value == original
        setattr(history, name, type(original)(value))
    global NATIVE_HISTORY_FUNCTIONS
    NATIVE_HISTORY_FUNCTIONS = {"plan": history.plan, "clock": history._clock}

    from tokeneconomizer.plugin import fold, denylist
    constants = invoke("fold.constants", {})
    for name, value in constants.items():
        original = getattr(fold, name)
        assert value == original if not isinstance(original, (tuple, frozenset)) else type(original)(value) == original
        setattr(fold, name, type(original)(value))
    global NATIVE_FOLD_FUNCTIONS, NATIVE_FOLD_DENY
    NATIVE_FOLD_FUNCTIONS = {name: getattr(fold, name) for name in ("fold_locked", "import_blocks")}
    NATIVE_FOLD_DENY = {name: getattr(denylist, name) for name in (*NATIVE_DENY_FUNCTIONS, "apply", "restore")}

    from tokeneconomizer.plugin import context, winspawn
    constants = invoke("winspawn.constants", {})
    assert constants["_FLAGS"] == winspawn._FLAGS and constants["_BREAKAWAY"] == winspawn._BREAKAWAY
    winspawn._FLAGS, winspawn._BREAKAWAY = constants["_FLAGS"], constants["_BREAKAWAY"]
    global NATIVE_SPAWN_FUNCTIONS
    NATIVE_SPAWN_FUNCTIONS = {"popen": subprocess.Popen, "run": subprocess.run, "call": subprocess.call, "wmi": winspawn._spawn_wmi}
    context.LiveContext.build = classmethod(context_build)
    for name in ("settle", "is_headless", "spawn"):
        setattr(context.LiveContext, name, wrap(getattr(context.LiveContext, name), "context." + name))
    context.LiveContext.mech_dir = property(wrap(context.LiveContext.mech_dir.fget, "context.mech_dir"))

    from tokeneconomizer.plugin import hosts
    constants = invoke("host_helpers.constants", {})
    assert constants["RESULT_CAP"] == hosts.RESULT_CAP and constants["PROGRESS"] == hosts.PROGRESS
    assert set(constants["HOSTS"]) == set(hosts.HOSTS)
    for cls in (hosts.Host, *hosts.HOSTS.values()):
        profile = invoke("host_helpers.profile", {"host": cls.name})
        for name, value in profile.items():
            assert getattr(cls, name) == value
            setattr(cls, name, value)
        for name in ("tool_call", "session_id", "turn_key", "env_session_id", "is_subagent", "context", "notice", "relay", "stop_hint", "stopped_normally", "completed_at_end"):
            if name in cls.__dict__:
                setattr(cls, name, wrap(getattr(cls, name), "host_helpers." + name))
        for name in ("cwd", "transcript", "headless", "mechanism_trace", "session_usage", "prompt_count", "settle", "complete", "last_active", "session_state", "tool_usage", "history", "log_head", "safe_session_id", "prompt_turns", "turn_calls", "turn_interrupted", "turn_answered", "folder_matches", "prepare", "record_event", "_state_path", "_load", "_save", "_prune", "expand", "_spool", "render"):
            if name in cls.__dict__:
                setattr(cls, name, wrap(getattr(cls, name), "host_runtime." + name))
    hosts.Antigravity._fresh = staticmethod(wrap(hosts.Antigravity._fresh, "host_runtime._fresh"))
    global NATIVE_HOST_ANCESTORS
    NATIVE_HOST_ANCESTORS = hosts._ancestor_args

    from tokeneconomizer.plugin.controller import LiveController
    constants = invoke("controller.constants", {})
    assert constants["TURN_HISTORY"] == importlib.import_module("tokeneconomizer.plugin.controller").TURN_HISTORY
    LiveController.__init__ = controller_constructor
    for name in ("_sess_name", "_session", "_save", "_cmd", "_log", "handler", "handle", "session_start", "_record_error", "_toolprune_notices", "_inject_map", "_close_turn", "user_prompt", "stop", "_judge", "post_tool", "session_end"):
        setattr(LiveController, name, wrap(getattr(LiveController, name), "controller." + name))
    global NATIVE_CONTROLLER_FUNCTIONS
    NATIVE_CONTROLLER_FUNCTIONS = {name: getattr(LiveController, name) for name in ("session_start", "user_prompt", "post_tool", "stop", "session_end")}

    from tokeneconomizer.loopguard.envelope import EnvelopeTracker
    EnvelopeTracker.__init__ = state_constructor(EnvelopeTracker, "envelope.new")
    EnvelopeTracker.update = wrap(EnvelopeTracker.update, "envelope.update")
    EnvelopeTracker.thresholds = wrap(EnvelopeTracker.thresholds, "envelope.tracker_thresholds")
    EnvelopeTracker.envelope = property(lambda self: invoke("envelope.envelope", {"self": vars(self)}))

    # Package re-exports and imports made before this plugin was configured keep aliases.
    for module in list(sys.modules.values()):
        if module and getattr(module, "__name__", "").startswith("tokeneconomizer"):
            for name, value in list(vars(module).items()):
                if inspect.isfunction(value) and value in replacements:
                    setattr(module, name, replacements[value])

    from tokeneconomizer.plugin import toolcheck
    global NATIVE_TOOLCHECK_FUNCTIONS
    NATIVE_TOOLCHECK_FUNCTIONS = {name: getattr(toolcheck, name) for name in (
        "claude_flags", "claude_cli", "program", "_prompt_tokens", "_codex_prompt_chars", "copilot_capture", "_init_tools", "_copilot_version", "_write_json")}
    from tokeneconomizer.plugin import hook
    global NATIVE_HOOK_RUN, NATIVE_HOOK_HOSTS
    NATIVE_HOOK_RUN = hook.run
    NATIVE_HOOK_HOSTS = {name: {method: getattr(cls, method, None) for method in (
        "cwd", "session_id", "prepare", "record_event", "expand", "render")}
        for name, cls in hosts.HOSTS.items()}


class NativeCliParser:
    """Translate native parser results into the original tests' argparse objects."""
    def __init__(self, data):
        from types import SimpleNamespace
        self.cwd = data["cwd"]
        self._subparsers = SimpleNamespace(_group_actions=[
            SimpleNamespace(choices=dict.fromkeys(data["commands"]))])

    def parse_args(self, args=None):
        return cli_call("cli.parse_args", {"argv": args, "cwd": self.cwd})

    def format_help(self):
        return cli_call("cli.format_help", {})

    def format_usage(self):
        return cli_call("cli.format_usage", {})

    def print_help(self, file=None):
        (sys.stdout if file is None else file).write(self.format_help())

    def print_usage(self, file=None):
        (sys.stdout if file is None else file).write(self.format_usage())


def cli_call(operation, arguments):
    from argparse import Namespace
    from types import SimpleNamespace
    import shutil
    from tokeneconomizer.plugin import cli, sidecall, denylist, history, repomap, fold, judge, repos, paths, toolcheck
    from tokeneconomizer.plugin.context import LiveContext
    arguments = dict(arguments)
    exceptions = {}
    ctx = arguments.get("ctx")
    host = getattr(ctx, "host", None)
    if "args" in arguments:
        arguments["args"] = {key: value for key, value in vars(arguments["args"]).items() if key != "fn"}
    if "ctx" in arguments:
        arguments["ctx"] = context_data(ctx)
    if "cfg" in arguments:
        arguments["cfg"] = dataclasses.asdict(arguments["cfg"])
    if "tracker" in arguments:
        arguments["tracker"] = vars(arguments["tracker"])
    if operation == "cli._session_note":
        arguments["env"] = dict(os.environ if arguments["env"] is None else arguments["env"])
    if operation in ("cli._expand_argline", "cli.main", "cli.parse_args"):
        arguments["argv"] = list(sys.argv[1:] if arguments["argv"] is None else arguments["argv"])
    if "path" in arguments:
        arguments["path"] = os.fspath(arguments["path"])
        if operation == "cli._unwritable":
            arguments["path"] = os.path.abspath(arguments["path"])
    if "cwd" in arguments:
        arguments["cwd"] = os.fspath(arguments["cwd"])
    arguments.update(pid=os.getpid(), argv=arguments.get("argv", sys.argv[1:]),
                     process_env=dict(os.environ), home=os.path.expanduser("~"), process_cwd=os.getcwd())
    result = invoke(operation, arguments)
    while isinstance(result, dict) and result.get("callback") == "cli":
        method, args = result["method"], result["args"]
        if method == "pending":
            result = invoke("cli.resume", {"job": result["job"]})
            continue
        try:
            if method == "context":
                ctx = cli._ctx(SimpleNamespace(**args))
                host = ctx.host
                value = context_data(ctx)
            elif method == "context_session":
                raw, session_id = args
                ctx = cli._ctx(SimpleNamespace(**raw), session_id=session_id)
                host = ctx.host
                value = context_data(ctx)
            elif method == "say":
                print(args)
                value = None
            elif method == "unwritable":
                value = cli._unwritable(*args)
            elif method == "global":
                name, params = args
                if name == "state_root":
                    value = paths.state_root()
                elif name == "stderr":
                    value = sys.stderr.write(*params)
                elif name == "execute":
                    raw = dict(params[0])
                    raw["fn"] = getattr(cli, raw["fn"])
                    value = raw["fn"](Namespace(**raw))
                elif name == "probe_run":
                    from tokeneconomizer.plugin import probe
                    value = probe.run(sessions=params[0], model=params[1], keep=params[2])
                elif name == "runtime":
                    value = {"name": "python", "version": sys.version, "parts": list(sys.version_info[:2]),
                             "minimum": [3, 10], "required": "3.10"}
                elif name == "claude_cli":
                    value = sidecall.claude_cli()
                elif name == "hermes_cli":
                    value = sidecall.hermes_cli()
                elif name == "which":
                    value = shutil.which(*params)
                elif name == "time":
                    value = cli.time.time()
                elif name == "pid":
                    value = os.getpid()
                elif name == "sleep":
                    value = cli.time.sleep(*params)
                elif name == "touch":
                    value = os.utime(*params)
                elif name == "refuse_unwritable":
                    value = cli._refuse_unwritable(*params)
                elif name == "repos_run_migration":
                    value = repos.run_migration(*params)
                elif name == "repos_merge":
                    if isinstance(params[2], dict):
                        params[2] = paths.RepoId(**params[2])
                    value = repos.merge(*params)
                elif name == "repos_refold":
                    value = repos.refold_moved(*params)
                elif name == "repos_listing":
                    value = repos.listing(params[0], current=paths.RepoId(**params[1]))
                else:
                    raise AssertionError("Unknown global CLI callback: " + name)
            elif method == "service":
                data, name, params = args
                if ctx is None:
                    ctx = LiveContext.__new__(LiveContext)
                restore_context(ctx, data, host)
                if name == "session_note":
                    value = cli._session_note(ctx)
                elif name == "paused":
                    value = sidecall.paused(host.name, ctx.root)
                elif name == "proposal":
                    value = cli._proposal(ctx, *params)
                elif name == "applied":
                    value = denylist.applied_for(ctx.store, ctx.repo.root, host.name)
                elif name == "kept":
                    value = denylist.kept_for(ctx.store, ctx.repo.root, host.name)
                elif name == "codex_trusted":
                    value = denylist.codex_project_trusted(ctx.repo.root)
                elif name == "copilot_trusted":
                    value = denylist.copilot_folder_trusted(ctx.repo.root)
                elif name == "copilot_folder_trusted":
                    value = denylist.copilot_folder_trusted(*params)
                elif name == "deny_apply":
                    value = denylist.apply(ctx.repo.root, params[0], ctx.store, host=host.name)
                elif name == "deny_restore":
                    value = denylist.restore(ctx.repo.root, ctx.store, params[0], host=host.name)
                elif name == "set_kept":
                    value = denylist.set_kept(ctx.store, ctx.repo.root, host.name, add=params[0], remove=params[1])
                elif name == "verify_copilot":
                    value = toolcheck.verify_copilot(ctx.repo.root, *params)
                elif name == "verify_codex":
                    value = toolcheck.verify_codex(ctx.repo.root, *params)
                elif name == "apply_pending":
                    value = fold.apply_pending(ctx)
                elif name == "refresh_catalog":
                    value = toolcheck.refresh_catalog(paths.state_root(), host=host.name)
                elif name == "measure":
                    value = toolcheck.measure(params[0], host=host.name)
                elif name == "history_summary":
                    value = history.summary(ctx)
                elif name == "repomap_state":
                    value = repomap.read_state(ctx)
                elif name == "repomap_refresh":
                    value = repomap.refresh(ctx)
                elif name == "repomap_python":
                    value = repomap.find_python(ctx)
                elif name == "head_commit":
                    value = repomap.head_commit(*params)
                elif name == "pending_sessions":
                    value = fold.pending_sessions(ctx)
                elif name == "fold_session":
                    value = fold.fold_session(ctx, *params)
                elif name == "judge_job":
                    value = judge.run_job(ctx, *params)
                elif name == "history_plan":
                    value = history.plan(ctx)
                elif name == "history_refusal":
                    value = history._refusal(ctx, *params)
                elif name == "history_run":
                    value = history.run(ctx, auto=params[0], log=lambda line: print(line, flush=True))
                elif name == "resume":
                    value = sidecall.resume(host.name, ctx.root)
                elif name == "call_name":
                    value = sidecall.call_name(host.name)
                else:
                    value = getattr(cli, name if name != "refuse_unwritable" else "_refuse_unwritable")(*params)
            else:
                raise AssertionError("Unknown CLI callback: " + method)
            reply = {"value": value}
        except BaseException as error:
            exceptions[type(error).__name__, str(error)] = error
            reply = {"error": str(error), "kind": type(error).__name__,
                     "parents": [base.__name__ for base in type(error).__mro__[1:]]}
        result = invoke("cli.resume", {"job": result["job"], "reply": reply})
    exception = exceptions.get((result.get("raised"), result.get("message")))
    if exception is not None:
        raise exception
    if result.get("raised") == "SystemExit":
        raise SystemExit(int(result["message"]))
    value = storage_result(result)
    if operation == "cli.build_parser":
        return NativeCliParser(value)
    if operation == "cli.parse_args":
        sys.stdout.write(value["stdout"])
        sys.stderr.write(value["stderr"])
        if value["exit"] is not None:
            raise SystemExit(value["exit"])
        parsed = value["args"]
        parsed["fn"] = getattr(cli, parsed["fn"])
        return Namespace(**parsed)
    if operation == "cli._session_ctx":
        restore_context(ctx, value, host)
        return ctx
    return value


def probe_call(operation, arguments):
    from tokeneconomizer.plugin import probe
    arguments = dict(arguments)
    out = arguments.pop("out", print)
    ctx = arguments.get("ctx")
    exceptions = {}
    if ctx is not None:
        arguments["ctx"] = context_data(ctx)
    arguments = {key: os.fspath(value) if isinstance(value, os.PathLike) else value
                 for key, value in arguments.items()}
    arguments.update(process_env=dict(os.environ), home=os.path.expanduser("~"),
                     process_cwd=os.getcwd(), python=sys.executable, module_file=probe.__file__)
    result = invoke(operation, arguments)
    while isinstance(result, dict) and result.get("callback") == "cli":
        method, args = result["method"], result["args"]
        if method == "pending":
            result = invoke("cli.resume", {"job": result["job"]})
            continue
        try:
            if method == "say":
                value = out(args)
            else:
                if method != "probe":
                    raise AssertionError("Unknown probe callback: " + method)
                name, params = args
                if name == "program":
                    value = probe.sidecall.program(*params)
                elif name == "process":
                    command, repo, env, timeout = params
                    completed = probe.subprocess.run(command, cwd=repo, env=env, timeout=timeout,
                        capture_output=True, encoding="utf-8", errors="replace", stdin=subprocess.DEVNULL)
                    value = {"stdout": completed.stdout, "stderr": completed.stderr}
                elif name == "git":
                    # The original fixtures isolate HOME; Git retains its configured signing identity.
                    git_env = dict(os.environ)
                    git_env.update(NATIVE_PROBE_GIT_ENV)
                    probe.subprocess.run(params, check=True, env=git_env)
                    value = None
                elif name == "settle":
                    value = probe.hosts.Copilot().settle(*params)
                elif name == "time":
                    value = probe.time.time()
                elif name == "sleep":
                    value = probe.time.sleep(*params)
                elif name == "which":
                    value = probe.shutil.which(*params)
                elif name == "copilot_model":
                    value = probe.sidecall.copilot_model(*params)
                elif name == "plugin_root":
                    value = probe._plugin_root()
                elif name == "trusted_folders":
                    value = probe.denylist.copilot_trusted_folders()
                elif name == "writable":
                    value = probe.os.access(params[0], os.W_OK)
                elif name == "mkdtemp":
                    value = probe.tempfile.mkdtemp(prefix=".te-probe-copilot-", dir=params[0])
                elif name == "make_repo":
                    value = probe.make_repo(*params)
                elif name == "refresh_catalog":
                    value = probe.toolcheck.refresh_catalog(*params, host="copilot")
                elif name == "context":
                    ctx = probe.LiveContext.build(probe.hosts.Copilot(), *params)
                    value = context_data(ctx)
                elif name == "run_session":
                    value = probe.run_session(*params[:4], cli=params[4])
                elif name == "wait_folded":
                    value = probe._wait_folded(ctx, params[1])
                elif name == "inspect_session":
                    value = probe.inspect_session(*params)
                elif name == "apply":
                    value = probe.denylist.apply(params[0], params[1], ctx.store, host="copilot")
                elif name == "restore":
                    value = probe.denylist.restore(params[0], ctx.store, host="copilot")
                else:
                    raise AssertionError("Unknown probe service: " + name)
            reply = {"value": value}
        except Exception as error:
            exceptions[type(error).__name__, str(error)] = error
            reply = {"error": str(error), "kind": type(error).__name__,
                     "parents": [base.__name__ for base in type(error).__mro__[1:]]}
        result = invoke("cli.resume", {"job": result["job"], "reply": reply})
    exception = exceptions.get((result.get("raised"), result.get("message")))
    if exception is not None:
        raise exception
    value = storage_result(result)
    if operation in ("probe.test_commands", "probe.make_repo", "probe.run_session"):
        return tuple(value)
    return value


def hook_call(operation, arguments):
    from tokeneconomizer.plugin import hook, hosts
    from tokeneconomizer.plugin.context import LiveContext
    from tokeneconomizer.plugin.controller import LiveController
    arguments = dict(arguments)
    if operation == "hook._truthy":
        return invoke(operation, arguments)
    stdout = arguments.pop("stdout", None)
    stdin = arguments.pop("stdin", None)
    argv = arguments.pop("argv", None)
    if operation == "hook.main":
        stdin = sys.stdin if stdin is None else stdin
        stdout = sys.stdout if stdout is None else stdout
        arguments["argv"] = sys.argv[1:] if argv is None else argv
        try:
            buffer = getattr(stdin, "buffer", None)
            raw = buffer.read() if buffer is not None else stdin.read().encode("utf-8")
            arguments["input"] = list(raw)
        except Exception:
            arguments["input_error"] = traceback.format_exc()
    if operation == "hook._record_error":
        arguments["error"] = traceback.format_exc()
    arguments["env"] = dict(os.environ if arguments.get("env") is None else arguments["env"])
    arguments.update(process_env=dict(os.environ), process_cwd=os.getcwd(), home=os.path.expanduser("~"), pid=os.getpid(),
                     patched_run=hook.run is not NATIVE_HOOK_RUN)
    executable = os.environ.get("TE_NATIVE_HOOK_TEST_EXEC")
    if (operation == "hook.main" and executable and "input" in arguments
            and not arguments["patched_run"]):
        process_env = {key: os.environ[key] for key in (
            "HOME", "USERPROFILE", "SystemRoot", "WINDIR", "COMSPEC", "PATHEXT",
            "TMPDIR", "TMP", "TEMP", "LANG", "LC_ALL", "TZ") if key in os.environ}
        process_env.update({key: str(value) for key, value in arguments["env"].items()})
        result = ORIGINAL_SUBPROCESS_RUN([executable, *arguments["argv"]],
            input=bytes(arguments["input"]), capture_output=True, env=process_env, timeout=30)
        if result.returncode:
            raise RuntimeError(result.stderr.decode("utf-8", errors="replace"))
        sys.stderr.write(result.stderr.decode("utf-8", errors="replace"))
        stdout.write(result.stdout.decode("utf-8"))
        stdout.flush()
        CALLS["native_hook.process"] += 1
        return 0
    host, ctx = None, None
    result = invoke(operation, arguments)
    while isinstance(result, dict) and result.get("callback") == "hook":
        method, args = result["method"], result["args"]
        if method == "pending":
            result = invoke("hook.resume", {"job": result["job"]})
            continue
        try:
            with contextlib.redirect_stdout(sys.stderr):
                if method == "detect":
                    payload, env, hint = args
                    host = hosts.detect(payload, env, hint=hint)
                    baseline = NATIVE_HOOK_HOSTS.get(host.name, {})
                    methods = {name: getattr(getattr(host, name, None), "__func__", getattr(host, name, None)) for name in (
                        "cwd", "session_id", "prepare", "record_event", "expand", "render")}
                    value = {"host": dict(name=host.name, keeps_history=host.keeps_history,
                        **{name: hasattr(host, name) for name in ("prepare", "record_event", "expand", "render")}),
                        "custom": {name: fn is not baseline.get(name) for name, fn in methods.items()}}
                elif method == "handle":
                    data, payload = args
                    if ctx is None:
                        ctx = LiveContext.__new__(LiveContext)
                    restore_context(ctx, data, host)
                    output = LiveController(ctx, payload).handle()
                    value = {"ctx": context_data(ctx), "output": output}
                elif method == "host":
                    name, payload, env, data, output = args
                    if data is not None:
                        if ctx is None:
                            ctx = LiveContext.__new__(LiveContext)
                        restore_context(ctx, data, host)
                    params = {"session_id": (payload,), "record_event": (payload,),
                              "render": (payload, output, ctx)}.get(name, (payload, env))
                    value = getattr(host, name)(*params)
                elif method == "run":
                    payload, env, hint = args
                    value = hook.run(payload, env, host_hint=hint)
                else:
                    raise AssertionError("Unknown hook callback: " + method)
            reply = {"value": value}
        except Exception:
            reply = {"error": traceback.format_exc()}
        result = invoke("hook.resume", {"job": result["job"], "reply": reply})
    value = storage_result(result)
    if operation == "hook.main":
        stdout.write(value)
        stdout.flush()
        return 0
    return value


def config_constructor(cls):
    return state_constructor(cls, "config." + cls.__name__)


def hookqueue_call(operation, arguments):
    from tokeneconomizer.plugin import hookqueue
    arguments = dict(arguments)
    ctrl = arguments.pop("ctrl", None)
    handler = arguments.pop("handler", None)
    if ctrl is not None:
        arguments.update(root=os.fspath(getattr(getattr(ctrl, "ctx", None), "root", ".")),
                         host=getattr(getattr(ctrl, "host", None), "name", ""),
                         session_id=getattr(ctrl, "sid", ""),
                         repomap=getattr(getattr(ctrl, "cfg", None), "repomap", ""))
        if operation == "hookqueue.run":
            arguments["payload"] = ctrl.p
    for name in ("base", "queue", "root"):
        if name in arguments:
            arguments[name] = os.path.abspath(arguments[name])
    arguments.update(wait=hookqueue.WAITS.get(arguments.get("event")), max_events=hookqueue.DRAIN_MAX_EVENTS,
                     budget=hookqueue.DRAIN_BUDGET_S, idle=hookqueue.IDLE_S,
                     enqueue_callback=hookqueue.enqueue is not NATIVE_QUEUE_ENQUEUE)
    result = invoke(operation, arguments)
    other, replay_handler, last_error = None, None, None
    while isinstance(result, dict) and result.get("callback") == "hookqueue":
        method, args = result["method"], result["args"]
        if method == "pending":
            result = invoke("hookqueue.resume", {"job": result["job"]})
            continue
        try:
            if method == "prepare_replay":
                other = type(ctrl)(ctrl.ctx, args[0])
                replay_handler = other.handler(args[1])
                value = {"session_id": other.sid, "handler": bool(replay_handler)}
            elif method == "replay_handler":
                other.replaying = args[0]
                replay_handler()
                value = None
            elif method == "record_error":
                try:
                    raise last_error
                except Exception:
                    ctrl._record_error()
                value = None
            elif method == "handler":
                value = handler()
            elif method == "enqueue":
                value = hookqueue.enqueue(*args)
            else:
                raise AssertionError("Unknown queue callback: " + method)
            reply = {"value": value}
        except Exception as error:
            last_error = error
            reply = {"error": str(error), "error_kind": type(error).__name__}
        result = invoke("hookqueue.resume", {"job": result["job"], "reply": reply})
    return storage_result(result)


def noted_runner(host, call):
    def run(prompt, model="agent"):
        return sidecall_call("sidecall.noted_call", {"host": host, "call": call, "prompt": prompt, "model": model})
    run.__wrapped__ = call
    return run


def host_callable(spec):
    from tokeneconomizer.plugin import sidecall
    kw = spec["args"]
    def claude(prompt, _m="agent"): return sidecall.claude_call(prompt, **kw)
    def codex(prompt, _m="agent"): return sidecall.codex_call(prompt, **kw)
    def copilot(prompt, _m="agent"): return sidecall.copilot_call(prompt, **kw)
    def pi(prompt, _m="agent"): return sidecall.pi_call(prompt, **kw)
    def opencode(prompt, _m="agent"): return sidecall.opencode_call(prompt, **kw)
    def cursor(prompt, _m="agent"): return sidecall.cursor_call(prompt, **kw)
    def grok(prompt, _m="agent"): return sidecall.grok_call(prompt, **kw)
    def hermes(prompt, _m="agent"): return sidecall.hermes_call(prompt, **kw)
    def droid(prompt, _m="agent"): return sidecall.droid_call(prompt, **kw)
    return {"claude_call": claude, "codex_call": codex, "copilot_call": copilot, "pi_call": pi,
            "opencode_call": opencode, "cursor_call": cursor, "grok_call": grok, "hermes_call": hermes, "droid_call": droid}[spec["method"]]


def routed_runner(state, fallback):
    def run(prompt, model="agent"):
        return sidecall_call("sidecall.runner_run", {"runner": run._native_state, "prompt": prompt,
                                                    "model": model, "fallback": fallback, "proxy": run})
    run._native_state = state
    if state["local"]:
        run.local, run.answered_locally = "fm", state["answered_locally"]
    elif state["note"] is not None:
        run.__wrapped__ = host_callable(state["host"])
    return run


def failure_data(error):
    if isinstance(error, subprocess.TimeoutExpired):
        return {"type": "timeout", "timeout": error.timeout}
    if isinstance(error, subprocess.CalledProcessError):
        return {"type": "exit", "returncode": error.returncode}
    return {"type": "exception", "name": type(error).__name__, "message": str(error)}


def sidecall_call(operation, arguments):
    from tokeneconomizer.plugin import sidecall, paths
    from tokeneconomizer.plugin import toolcheck
    namespace = "toolcheck" if operation.startswith("toolcheck.") else "sidecall"
    api = toolcheck if namespace == "toolcheck" else sidecall
    arguments = dict(arguments)
    if operation == "sidecall._noted":
        return noted_runner(arguments["host"], arguments["call"])
    callback = arguments.pop("call", None)
    fallback = arguments.pop("fallback", None)
    proxy = arguments.pop("proxy", None)
    arguments["has_fallback"] = fallback is not None
    called_value = None
    probe_process = arguments.pop("p", None)
    if probe_process is not None:
        arguments["pid"] = probe_process.pid
    probe_lines = None
    if "config" in arguments:
        config = arguments["config"]
        arguments["config"] = {key: getattr(config, key) for key in LIVE_CONFIG_FIELDS if hasattr(config, key)}
    if "env" not in arguments or arguments["env"] is None:
        arguments["env"] = dict(os.environ)
    if "root" not in arguments or not arguments["root"]:
        arguments["root"] = paths.state_root()
    arguments["root"] = os.fspath(arguments["root"])
    if "path" in arguments:
        arguments["path"] = os.fspath(arguments["path"])
    if "out" in arguments:
        out = arguments.pop("out")
        arguments.update(stdout=getattr(out, "stdout", "") or "", stderr=getattr(out, "stderr", "") or "",
                         code=getattr(out, "returncode", "?"))
    arguments.update(home=os.path.expanduser("~"), process_cwd=os.getcwd(), windows=os.name == "nt", macos=sidecall._macos())
    arguments.update(help=dict(sidecall._HELP), oc_flags=dict(sidecall._OC_FLAGS), fm_ok=dict(sidecall._fm_ok), transport={"run": subprocess.run is not NATIVE_SPAWN_FUNCTIONS["run"],
                                                       "hermes_cli": sidecall.hermes_cli is not NATIVE_SIDECALL_FUNCTIONS["hermes_cli"],
                                                       "effective": sidecall.accounting.effective is not NATIVE_SIDECALL_FUNCTIONS["effective"],
                                                       "access": sidecall.os.access is not NATIVE_SIDECALL_FUNCTIONS["access"],
                                                       "program": api.program is not NATIVE_SIDECALL_FUNCTIONS["program"],
                                                       "temporary_directory": sidecall.tempfile.TemporaryDirectory is not NATIVE_SIDECALL_FUNCTIONS["temporary_directory"],
                                                       "claude_cli": api.claude_cli is not NATIVE_SIDECALL_FUNCTIONS["claude_cli"]})
    arguments["hermes_limits"] = {"delete_timeout": sidecall.HERMES_DELETE_TIMEOUT_S,
                                  "title_wait": sidecall.HERMES_TITLE_WAIT_S, "abandon": sidecall.HERMES_ABANDON_S}
    arguments["transport"].update({name: getattr(sidecall, name) is not original for name, original in NATIVE_SIDECALL_FUNCTIONS.items()
                                  if name.endswith("_call") or name == "fm_available"})
    if namespace == "toolcheck":
        arguments["transport"]["popen"] = subprocess.Popen is not NATIVE_SPAWN_FUNCTIONS["popen"]
        arguments["transport"].update({name: getattr(toolcheck, name) is not original
                                      for name, original in NATIVE_TOOLCHECK_FUNCTIONS.items()})
        for name in ("tools", "servers", "capabilities", "extra_args", "overrides"):
            if name in arguments:
                arguments[name] = list(arguments[name])
        for name in ("cwd", "state_root", "repo_root"):
            if name in arguments and arguments[name] is not None:
                arguments[name] = os.fspath(arguments[name])
    last_error = None
    temporary = None
    result = invoke(operation, arguments)
    while isinstance(result, dict) and result.get("callback") == namespace:
        if result["method"] == "pending":
            result = invoke(namespace + (".resume_job" if namespace == "sidecall" else ".resume"), {"job": result["job"]})
            continue
        try:
            if result["method"] == "noted":
                called_value = callback(*result["args"])
                value = None
            elif result["method"] == "model_call":
                name, data = result["args"]
                value = getattr(sidecall, name)(data["prompt"], **data["kwargs"])
            elif result["method"] == "probe":
                name, data = result["args"]
                value = getattr(toolcheck, name)(**data)
            elif result["method"] == "probe_start":
                argv, cwd, env, new_session = result["args"]
                probe_process = subprocess.Popen(argv, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                                                 stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                                                 encoding="utf-8", errors="replace", start_new_session=new_session)
                probe_lines = queue.Queue()
                def read_probe():
                    try:
                        for line in probe_process.stdout:
                            probe_lines.put({"value": line})
                    except Exception as error:
                        probe_lines.put({"error": str(error)})
                    finally:
                        probe_lines.put({"value": None})
                threading.Thread(target=read_probe, daemon=True).start()
                value = probe_process.pid
            elif result["method"] == "probe_line":
                try:
                    line = probe_lines.get(timeout=result["args"][0])
                except queue.Empty:
                    line = {"value": None}
                if "error" in line:
                    raise OSError(line["error"])
                value = line["value"]
            elif result["method"] == "probe_stop":
                probe_process.kill()
                probe_process.wait()
                value = None
            elif result["method"] == "fm_available":
                value = sidecall.fm_available()
            elif result["method"] == "fallback":
                value = fallback(*result["args"])
            elif result["method"] == "which":
                value = sidecall.shutil.which(*result["args"])
            elif result["method"] == "claude_cli":
                value = api.claude_cli()
            elif result["method"] == "hermes_cli":
                value = sidecall.hermes_cli()
            elif result["method"] == "effective":
                value = sidecall.accounting.effective(*result["args"])
            elif result["method"] == "program":
                value = api.program(*result["args"])
            elif result["method"] == "access":
                value = sidecall.os.access(*result["args"])
            elif result["method"] == "temporary_directory":
                prefix, ignore_cleanup_errors = result["args"]
                temporary = sidecall.tempfile.TemporaryDirectory(prefix=prefix, ignore_cleanup_errors=ignore_cleanup_errors)
                value = temporary.name
            elif result["method"] == "remove_temporary_directory":
                temporary.cleanup()
                value = None
            elif result["method"] == "run":
                argv, cwd, env, stdin, timeout = result["args"]
                kw = {"input": stdin} if stdin is not None else {"stdin": subprocess.DEVNULL}
                out = subprocess.run(argv, cwd=cwd, env=env, capture_output=True, encoding="utf-8", errors="replace", timeout=timeout, **kw)
                value = {"code": out.returncode, "stdout": out.stdout or "", "stderr": out.stderr or ""}
            else:
                raise AssertionError("Unknown side-call callback: " + result["method"])
            reply = {"value": value}
        except Exception as error:
            last_error = error
            reply = {"error": str(error), "error_kind": type(error).__name__, "failure": failure_data(error)}
        result = invoke(namespace + (".resume_job" if namespace == "sidecall" else ".resume"), {"job": result["job"], "reply": reply})
    if isinstance(result, dict) and "help" in result:
        sidecall._HELP.clear()
        sidecall._HELP.update(result["help"])
    if isinstance(result, dict) and "oc_flags" in result:
        sidecall._OC_FLAGS.clear()
        sidecall._OC_FLAGS.update(result["oc_flags"])
    if isinstance(result, dict) and "fm_ok" in result:
        sidecall._fm_ok.clear()
        sidecall._fm_ok.update(result["fm_ok"])
    if proxy is not None and isinstance(result, dict) and "runner" in result:
        proxy._native_state = result["runner"]
        if result["runner"]["local"]:
            proxy.answered_locally = result["runner"]["answered_locally"]
    if last_error is not None and isinstance(result, dict) and "raised" in result and result.get("message") == str(last_error):
        raise last_error
    if last_error is not None and isinstance(result, dict) and "raised" in result and "failure" in result:
        raise last_error
    value = storage_result(result)
    if operation in ("sidecall._host_call", "sidecall.local_runner", "sidecall.runner_for"):
        return routed_runner(result["runner"], fallback) if value else None
    if operation == "sidecall.noted_call":
        if result["failed"]:
            raise last_error
        return called_value
    if operation == "sidecall._hermes_ledger":
        from tokeneconomizer.plugin.store import Store
        return Store(value)
    return tuple(value) if operation in ("sidecall.claude_call", "sidecall.codex_call", "sidecall.pi_call", "sidecall.cursor_call", "sidecall.grok_call", "sidecall.opencode_call", "sidecall.copilot_call", "sidecall.droid_call", "sidecall.fm_call", "sidecall.hermes_call", "sidecall._hermes_settle", "sidecall.runner_run", "toolcheck._prompt_tokens", "toolcheck._init_tools", "toolcheck.refresh_catalog") else value


def repomap_call(operation, arguments):
    from tokeneconomizer.plugin import repomap
    arguments = dict(arguments)
    ctx = arguments.pop("ctx", None)
    if ctx is not None:
        arguments["ctx"] = context_data(ctx)
    arguments.update(process_env=dict(os.environ), process_cwd=os.getcwd(), home=os.path.expanduser("~"),
                     generator=str(ROOT / "target/debug/te-repomap-gen"), interpreter=sys.executable,
                     transport={"run": subprocess.run is not NATIVE_SPAWN_FUNCTIONS["run"]})
    for name, value in tuple(arguments.items()):
        if isinstance(value, os.PathLike):
            arguments[name] = os.fspath(value)
    if "idents" in arguments:
        arguments["idents"] = list(arguments["idents"])
    last_error = None
    result = invoke(operation, arguments)
    while isinstance(result, dict) and result.get("callback") == "repomap":
        if result["method"] == "pending":
            result = invoke("repomap.resume", {"job": result["job"]})
            continue
        try:
            if result["method"] == "which":
                value = repomap.shutil.which(*result["args"])
            elif result["method"] == "spawn":
                value = ctx.spawn(*result["args"])
            elif result["method"] == "run":
                argv, cwd, env, stdin, timeout = result["args"]
                out = subprocess.run(argv, cwd=cwd, env=env, capture_output=True, encoding="utf-8",
                                     errors="replace", timeout=timeout)
                value = {"code": out.returncode, "stdout": out.stdout or "", "stderr": out.stderr or ""}
            else:
                raise AssertionError("Unknown repository map callback: " + result["method"])
            reply = {"value": value}
        except Exception as error:
            last_error = error
            reply = {"error": str(error), "error_kind": type(error).__name__, "failure": failure_data(error)}
        result = invoke("repomap.resume", {"job": result["job"], "reply": reply})
    if last_error is not None and isinstance(result, dict) and result.get("message") == str(last_error):
        raise last_error
    value = storage_result(result)
    return tuple(value) if operation in ("repomap.generate", "repomap.for_prompt") else value


def controller_data(ctl):
    return {"ctx": context_data(ctl.ctx), "p": ctl.p, "sid": ctl.sid, "replaying": ctl.replaying}


def restore_controller(ctl, data, host):
    restore_context(ctl.ctx, data["ctx"], host)
    ctl.p, ctl.sid, ctl.replaying = data["p"], data["sid"], data["replaying"]
    ctl.host, ctl.store, ctl.cfg = host, ctl.ctx.store, ctl.ctx.config


def controller_constructor(self, ctx, payload):
    self.ctx, self.host = ctx, ctx.host
    data = controller_call("controller.new", {"ctx": ctx, "payload": payload})
    restore_controller(self, data, ctx.host)


def controller_call(operation, arguments):
    from tokeneconomizer.plugin import history, fold, repos, sidecall, repomap, paths, hookqueue, hosts, judge
    arguments = dict(arguments)
    obj = arguments.pop("self", None)
    ctx = arguments.pop("ctx", getattr(obj, "ctx", None))
    host = getattr(obj, "host", getattr(ctx, "host", None))
    originals = {key: arguments.get(key) for key in ("s", "seen")}
    if operation == "controller.new":
        arguments["ctx"] = context_data(ctx)
    elif operation == "controller._judge":
        arguments["host"] = host.name
    elif operation != "controller.handler":
        arguments["self"] = controller_data(obj)
    if operation == "controller._record_error":
        arguments["error"] = traceback.format_exc()
    arguments.update(process_env=dict(os.environ), process_cwd=os.getcwd(), home=os.path.expanduser("~"),
                     parent_pid=os.getppid(), windows=os.name == "nt")
    flags = {name: obj is not None and getattr(getattr(obj, name, None), "__func__", getattr(obj, name, None)) is not native
             for name, native in NATIVE_CONTROLLER_FUNCTIONS.items()}
    flags.update(enqueue=hookqueue.enqueue is not NATIVE_QUEUE_ENQUEUE,
                 process_run=subprocess.run is not NATIVE_SPAWN_FUNCTIONS["run"],
                 ancestors=hosts._ancestor_args is not NATIVE_HOST_ANCESTORS)
    arguments["transport"] = flags
    event = getattr(obj, "p", {}).get("hook_event_name", "")
    arguments["queue_limits"] = {"wait": hookqueue.WAITS.get(event), "max_events": hookqueue.DRAIN_MAX_EVENTS,
                                  "budget": hookqueue.DRAIN_BUDGET_S, "idle": hookqueue.IDLE_S}
    runners = {}
    result = invoke(operation, arguments)
    while isinstance(result, dict) and result.get("callback") == "controller":
        method, args = result["method"], result["args"]
        if isinstance(args, dict) and "ctx" in args:
            if args["ctx"] is not None:
                restore_context(ctx, args["ctx"], host)
            args = args["args"]
        if method == "pending":
            result = invoke("controller.resume", {"job": result["job"]})
            continue
        try:
            if method == "handler":
                data, name = args
                if data["replaying"]:
                    target = type(obj).__new__(type(obj))
                    target.ctx = ctx
                else:
                    target = obj
                restore_controller(target, data, host)
                value = getattr(target, name)()
            elif method in ("paused", "paused_bool"):
                value = sidecall.paused(host.name, ctx.root)
                if method == "paused_bool":
                    value = bool(value)
            elif method == "call_name":
                value = sidecall.call_name(host.name)
            elif method == "local_routes":
                value = bool(sidecall.local_routes("judge", ctx.config))
            elif method in ("launcher", "map", "history_notice", "history_start", "migration_start", "pending_sessions", "folds"):
                data, params = args
                restore_context(ctx, data, host)
                if method == "launcher":
                    value = paths.ensure_launcher(ctx.env)
                elif method == "map":
                    name, params = params
                    value = getattr(repomap, name)(ctx, *params)
                elif method == "history_notice":
                    value = history.notice(ctx)
                elif method == "history_start":
                    value = history.maybe_start(ctx)
                elif method == "migration_start":
                    value = repos.start_migration(ctx)
                elif method == "pending_sessions":
                    value = fold.pending_sessions(ctx, exclude=params[0])
                else:
                    value = fold.spawn_fold(ctx, params[0])
            elif method == "enqueue":
                value = hookqueue.enqueue(*args)
            elif method == "spawn_job":
                value = judge.spawn_job(ctx, *args)
            elif method == "spawn":
                value = ctx.spawn(*args)
            elif method == "program":
                value = list(sidecall.program(*args))
            elif method == "ancestors":
                value = list(hosts._ancestor_args())
            elif method == "process_run":
                argv, cwd, env, timeout = args
                out = subprocess.run(argv, capture_output=True, encoding="utf-8", errors="replace", timeout=timeout)
                value = {"code": out.returncode, "stdout": out.stdout or "", "stderr": out.stderr or ""}
            elif method == "runner":
                role, local_only = args
                runners[role] = sidecall.local_runner(role, ctx.config) if local_only else sidecall.runner_for(host.name, ctx.config, role=role)
                value = runners[role] is not None
            elif method == "run":
                value = runners[args[0]](*args[1:])
            elif method == "estimate":
                value = runners[args[0]].estimate_effective(*args[1:])
            elif method == "local_answer":
                runner = runners[args[0]]
                value = {"answered": bool(getattr(runner, "answered_locally", False)), "local": getattr(runner, "local", "local")}
            else:
                raise AssertionError("Unknown controller callback: " + method)
            reply = {"value": value}
        except Exception:
            reply = {"error": traceback.format_exc()}
        result = invoke("controller.resume", {"job": result["job"], "reply": reply})
    if obj is not None and isinstance(result, dict) and "self" in result:
        restore_controller(obj, result["self"], host)
    value = storage_result(result)
    if operation == "controller.handler":
        return getattr(obj, value) if value else None
    if operation == "controller._judge":
        if value == "none":
            return None
        if value == "no_verdict":
            return judge.NoVerdict()
        return judge.AsyncJudge(ctx, obj.sid, arguments["thr"].get("judge_confidence"))
    if operation in ("controller._save", "controller._toolprune_notices", "controller._inject_map", "controller._close_turn"):
        for key, original in originals.items():
            if original is not None and key in value:
                original.clear()
                original.update(value[key])
        return value["value"]
    return value


def host_session_call(operation, arguments):
    from tokeneconomizer.plugin import hosts, sidecall
    arguments = dict(arguments)
    host = arguments.pop("self", None)
    ctx = arguments.pop("ctx", None)
    original_out = arguments.get("out")
    arguments["ctx_env"] = dict(getattr(ctx, "env", {}))
    if host is not None:
        arguments["host"] = host.name
        arguments["limits"] = {"export_tries": getattr(host, "EXPORT_TRIES", 3),
                               "export_wait": getattr(host, "EXPORT_WAIT_S", 2.0),
                               "fold_retry": getattr(host, "FOLD_RETRY_S", 7 * 86400)}
        arguments.update(state_dir=getattr(host, "STATE_DIR", "antigravity-sessions"),
                         state_keep=getattr(host, "STATE_KEEP_S", 30 * 86400),
                         context_cap=getattr(host, "CONTEXT_CAP", 10000))
    arguments.update(process_env=dict(os.environ), process_cwd=os.getcwd(), home=os.path.expanduser("~"),
                     parent_pid=os.getppid(), windows=os.name == "nt")
    arguments["transport"] = {"run": subprocess.run is not NATIVE_SPAWN_FUNCTIONS["run"],
                              "ancestors": hosts._ancestor_args is not NATIVE_HOST_ANCESTORS}
    if "env" in arguments:
        arguments["env"] = dict(os.environ if arguments["env"] is None else arguments["env"])
    if "extra_dirs" in arguments:
        arguments["extra_dirs"] = [os.fspath(path) for path in arguments["extra_dirs"] or ()]
    if "turn_lines" in arguments:
        arguments["turn_lines"] = list(arguments["turn_lines"])
    for name, value in list(arguments.items()):
        if isinstance(value, os.PathLike):
            arguments[name] = os.fspath(value)
    result = invoke(operation, arguments)
    while isinstance(result, dict) and result.get("callback") == "host_runtime":
        method, args = result["method"], result["args"]
        if method == "pending":
            result = invoke("host_runtime.resume", {"job": result["job"]})
            continue
        try:
            if method == "program":
                value = list(sidecall.program(*args))
            elif method == "ancestors":
                value = list(hosts._ancestor_args())
            elif method == "run":
                argv, cwd, env, timeout = args
                kwargs = dict(capture_output=True, encoding="utf-8", errors="replace", timeout=timeout)
                if env is not None:
                    kwargs.update(cwd=cwd, env=env, stdin=subprocess.DEVNULL)
                out = subprocess.run(argv, **kwargs)
                value = {"code": out.returncode, "stdout": out.stdout or "", "stderr": out.stderr or ""}
            else:
                raise AssertionError("Unknown host session callback: " + method)
            reply = {"value": value}
        except Exception as error:
            reply = {"error": str(error), "error_kind": type(error).__name__}
        result = invoke("host_runtime.resume", {"job": result["job"], "reply": reply})
    if result.get("raised") == "FoldLater":
        raise hosts.FoldLater(result["message"])
    value = storage_result(result)
    if operation == "host_runtime.render" and host.name == "grok":
        original_out.clear()
        original_out.update(value["out"])
        return original_out
    if operation == "host_runtime.prompt_turns":
        return iter(value)
    if operation == "host_runtime.turn_calls":
        return [tuple(row) for row in value]
    if operation == "host_runtime.tool_usage":
        return collections.Counter(value)
    return value


def host_call(operation, arguments):
    from tokeneconomizer.plugin import hosts
    arguments = dict(arguments)
    host = arguments.pop("self", None)
    if operation in ("host_helpers.default_host", "host_helpers.detect"):
        arguments["registered"] = list(hosts.HOSTS)
    if host is not None:
        arguments.update(host=host.name, interrupt=host.interrupt, end_run=host.end_run)
    if "env" in arguments:
        environment = arguments["env"]
        if environment is None:
            environment = os.environ if operation == "host_helpers.default_host" else {}
        arguments["env"] = dict(environment)
    for name, value in list(arguments.items()):
        if isinstance(value, os.PathLike):
            arguments[name] = os.fspath(value)
    value = invoke(operation, arguments)
    if operation == "host_helpers.detect":
        return hosts.HOSTS[value]()
    return tuple(value) if operation == "host_helpers.tool_call" else value


def fold_call(operation, arguments):
    from tokeneconomizer.plugin import fold, denylist, hosts, sidecall
    arguments = dict(arguments)
    ctx = arguments.pop("ctx", None)
    doc = arguments.get("doc")
    arguments["ctx"] = context_data(ctx)
    arguments["lock_wait"] = fold.FOLD_LOCK_WAIT_S
    arguments["transport"] = {name: getattr(fold, name) is not original for name, original in NATIVE_FOLD_FUNCTIONS.items()}
    arguments["transport"]["deny"] = any(getattr(denylist, name) is not original for name, original in NATIVE_FOLD_DENY.items())
    for name, value in list(arguments.items()):
        if isinstance(value, os.PathLike):
            arguments[name] = os.fspath(value)
    for name in ("ids", "items", "proposal", "session_ids", "exclude", "servers"):
        if name in arguments:
            arguments[name] = list(arguments[name])
    if "cfg" in arguments:
        arguments["cfg"] = dataclasses.asdict(arguments["cfg"])
    if "adapter" in arguments:
        arguments["adapter"] = live_adapter_data(arguments["adapter"])
    if "stage" in arguments:
        arguments["stage"] = tooling_data(arguments["stage"])
    runner, cancelled = None, None
    result = invoke(operation, arguments)
    while isinstance(result, dict) and result.get("callback") == "fold":
        method, args = result["method"], result["args"]
        if method == "pending":
            result = invoke("fold.resume", {"job": result["job"]})
            continue
        try:
            if method == "host":
                name, params = args
                value = getattr(ctx.host, name)(*params)
            elif method == "proposer":
                runner = sidecall.local_runner("proposer", ctx.config) if sidecall.paused(ctx.host.name, ctx.root) else sidecall.runner_for(ctx.host.name, ctx.config, role="proposer")
                value = runner is not None
            elif method == "run":
                value = runner(*args)
            elif method == "estimate":
                value = runner.estimate_effective(*args)
            elif method == "spawn":
                value = ctx.spawn(*args)
            elif method == "fold_locked":
                value = fold.fold_locked(ctx, *args)
            elif method == "import_blocks":
                value = fold.import_blocks(ctx)
            elif method == "apply":
                value = denylist.apply(args[0], args[1], ctx.store, host=ctx.host.name)
            elif method == "restore":
                value = denylist.restore(ctx.repo.root, ctx.store, args[0], host=ctx.host.name)
            elif method == "git_tracked":
                value = denylist._git_tracked(*args)
            else:
                raise AssertionError("Unknown fold callback: " + method)
            reply = {"value": value}
        except BaseException as error:
            fatal = not isinstance(error, Exception)
            if fatal:
                cancelled = error
            reply = {"error": str(error), "error_kind": type(error).__name__, "fatal": fatal}
        result = invoke("fold.resume", {"job": result["job"], "reply": reply})
    if result.get("cancelled"):
        if cancelled is None:
            raise AssertionError("A fold cancelled without a source exception")
        raise cancelled
    if result.get("raised") == "Busy":
        raise fold.Busy(result["message"])
    if result.get("raised") == "FoldLater":
        raise hosts.FoldLater(result["message"])
    value = storage_result(result)
    if operation == "fold._hold":
        doc.clear()
        doc.update(value["doc"])
        return value["value"]
    if operation == "fold._microskills_config":
        from tokeneconomizer.config import InjectionConfig
        return InjectionConfig(**value)
    if operation == "fold.forget_microskills":
        return [tuple(row) for row in value]
    return value


def history_call(operation, arguments):
    from tokeneconomizer.plugin import history, fold
    arguments = dict(arguments)
    ctx = arguments.pop("ctx", None)
    store = arguments.pop("store", getattr(ctx, "store", None))
    log, change = arguments.pop("log", None), arguments.pop("change", None)
    original_doc, original_skipped, original_cache = arguments.get("doc"), arguments.get("skipped"), arguments.get("cache")
    aliased_skipped = isinstance(original_doc, dict) and original_doc.get("skipped") is original_skipped and original_skipped is not None
    arguments["ctx"] = context_data(ctx, store)
    arguments["log"] = log is not None
    arguments["limits"] = {"max_bytes": history.MAX_BYTES, "max_attempts": history.MAX_ATTEMPTS,
                           "heartbeat_stale": history.HEARTBEAT_STALE_S, "history_lock": history.HISTORY_LOCK_S,
                           "fold_lock": fold.FOLD_LOCK_WAIT_S, "stale": fold.STALE_S}
    arguments["transport"] = {name: getattr(history, name if name == "plan" else "_clock") is not original
                              for name, original in NATIVE_HISTORY_FUNCTIONS.items()}
    for name, value in list(arguments.items()):
        if isinstance(value, os.PathLike):
            arguments[name] = os.fspath(value)
    if operation == "history._record_error":
        arguments["error"] = traceback.format_exc()
    result, cancelled = invoke(operation, arguments), None
    while isinstance(result, dict) and result.get("callback") == "history":
        method, args = result["method"], result["args"]
        if method == "pending":
            result = invoke("history.resume", {"job": result["job"]})
            continue
        try:
            if method == "host":
                name, params = args
                value = getattr(ctx.host, name)(ctx.env, *params) if name == "history" else getattr(ctx.host, name)(*params)
                if isinstance(value, Iterator):
                    value = list(value)
            elif method == "plan":
                value = history.plan(ctx, now=args[0])
            elif method == "fold":
                sid, path, imported, span = args
                value = fold.fold_locked(ctx, sid, path, imported=True, span=tuple(span)) if imported else fold.fold_locked(ctx, sid)
            elif method == "pending_sessions":
                value = fold.pending_sessions(ctx)
            elif method == "spawn":
                value = ctx.spawn(*args)
            elif method == "log":
                log(*args)
                value = None
            elif method == "clock":
                value = history._clock()
            elif method == "change":
                doc = args[0]
                value = {"value": change(doc), "doc": doc}
            else:
                raise AssertionError("Unknown history callback: " + method)
            reply = {"value": value}
        except BaseException as error:
            fatal = not isinstance(error, Exception)
            if fatal:
                cancelled = error
            reply = {"error": traceback.format_exc(), "fatal": fatal}
        result = invoke("history.resume", {"job": result["job"], "reply": reply})
    if isinstance(result, dict) and result.get("cancelled"):
        if cancelled is None:
            raise AssertionError("A history job cancelled without a source exception")
        raise cancelled
    value = storage_result(result)
    if operation in ("history._identity", "history.is_this_repository"):
        if original_cache is not None:
            original_cache.clear()
            original_cache.update(value["cache"])
        return value["value"]
    if operation in ("history._run_locked", "history._import_one"):
        original_doc.clear()
        original_doc.update(value["doc"])
        if original_skipped is not None:
            original_skipped.clear()
            original_skipped.update(value["skipped"])
            if aliased_skipped:
                original_doc["skipped"] = original_skipped
        return value["value"]
    return value


def context_data(ctx, store=None):
    store = store if store is not None else getattr(ctx, "store", None)
    data = {"root": os.fspath(getattr(ctx, "root", ".")), "base": os.fspath(getattr(store, "base", ".")),
            "host": getattr(getattr(ctx, "host", None), "name", "claude"),
            "keeps_history": bool(getattr(getattr(ctx, "host", None), "keeps_history", False)),
            "config": dataclasses.asdict(ctx.config) if ctx is not None and hasattr(ctx, "config") else {},
            "env": dict(getattr(ctx, "env", {})), "home": os.path.expanduser("~"),
            "unsettled": getattr(ctx, "unsettled", None)}
    if ctx is not None and hasattr(ctx, "repo"):
        data["repo"] = dataclasses.asdict(ctx.repo)
    return data


def restore_context(ctx, data, host):
    from tokeneconomizer.plugin.paths import RepoId
    from tokeneconomizer.plugin.store import Store
    from tokeneconomizer.plugin.config import LiveConfig
    ctx.host, ctx.repo, ctx.root, ctx.env = host, RepoId(**data["repo"]), data["root"], data["env"]
    ctx.store = Store.__new__(Store)
    ctx.store.base = data["base"]
    cfg = getattr(ctx, "config", None)
    if cfg is None:
        cfg = LiveConfig.__new__(LiveConfig)
    cfg.__dict__.clear()
    cfg.__dict__.update(data["config"])
    ctx.config = cfg
    ctx.unsettled = tuple(data["unsettled"]) if data["unsettled"] is not None else None


def context_build(cls, host, cwd, env=None, session_id=None, settle=True):
    data = context_call("context.build", {"host": host, "cwd": cwd, "env": env, "session_id": session_id, "settle": settle})
    ctx = cls.__new__(cls)
    restore_context(ctx, data, host)
    return ctx


def context_call(operation, arguments):
    from tokeneconomizer.plugin import context, winspawn
    arguments = dict(arguments)
    ctx = arguments.get("self")
    host = ctx.host if ctx is not None else arguments.get("host")
    if operation == "context.build":
        arguments.update(host=host.name, keeps_history=bool(getattr(host, "keeps_history", False)),
                         env=dict(os.environ if arguments["env"] is None else arguments["env"]))
    if ctx is not None:
        arguments["self"] = context_data(ctx)
    if operation.startswith("winspawn.") and "cmd" in arguments:
        arguments["spec"] = {"cmd": list(arguments.pop("cmd")), "cwd": os.fspath(arguments.pop("cwd")),
                             "env": dict(arguments.pop("env")), "log": os.fspath(arguments.pop("log_path"))}
    arguments.update(home=os.path.expanduser("~"), executable=winspawn.sys.executable,
                     core=str(Path(context.__file__).absolute().parents[2]), runner=os.path.abspath(winspawn.__file__),
                     windows=context.os.name == "nt")
    arguments["transport"] = {name: (getattr(winspawn, "_spawn_wmi") if name == "wmi" else getattr(subprocess, {"popen": "Popen"}.get(name, name))) is not original
                              for name, original in NATIVE_SPAWN_FUNCTIONS.items()}
    for name, value in list(arguments.items()):
        if isinstance(value, os.PathLike):
            arguments[name] = os.fspath(value)
    result = invoke(operation, arguments)
    while isinstance(result, dict) and result.get("callback") == "context":
        method, args = result["method"], result["args"]
        if method == "pending":
            result = invoke("context.resume", {"job": result["job"]})
            continue
        try:
            if method == "headless":
                value = host.headless(*args)
            elif method == "popen":
                spec, flags, new_session = args
                with open(spec["log"], "a", encoding="utf-8") as log:
                    kwargs = dict(stdin=subprocess.DEVNULL, stdout=log, stderr=log, cwd=spec["cwd"], env=spec["env"])
                    if new_session:
                        kwargs["start_new_session"] = True
                    else:
                        kwargs.update(creationflags=flags, close_fds=True)
                    subprocess.Popen(spec["cmd"], **kwargs)
                value = None
            elif method == "powershell":
                value = subprocess.run(["powershell", "-NoProfile", "-NonInteractive", "-Command", args[0]],
                                       stdin=subprocess.DEVNULL, capture_output=True, timeout=15, creationflags=0x08000000).returncode
            elif method == "wmi":
                spec = args[0]
                value = winspawn._spawn_wmi(spec["cmd"], spec["cwd"], spec["env"], spec["log"])
            elif method == "call":
                spec, flags = args
                with open(spec["log"], "a", encoding="utf-8") as log:
                    value = subprocess.call(spec["cmd"], stdin=subprocess.DEVNULL, stdout=log, stderr=log,
                                            cwd=spec["cwd"], env=spec["env"], creationflags=flags)
            else:
                raise AssertionError("Unknown context callback: " + method)
            reply = {"value": value}
        except Exception as error:
            reply = {"error": str(error), "error_kind": type(error).__name__}
        result = invoke("context.resume", {"job": result["job"], "reply": reply})
    if ctx is not None and isinstance(result, dict) and "self" in result:
        restore_context(ctx, result["self"], host)
    return storage_result(result)


def live_judge_call(operation, ctx=None, session_id="", job_id="", threshold=None, guard=None, digest=""):
    from tokeneconomizer.plugin import judge, sidecall
    from tokeneconomizer.loopguard import judge as loop_judge
    store = ctx.store if ctx is not None else None
    flags = {}
    if store is not None:
        for name, original in NATIVE_JUDGE_STORE.items():
            method = getattr(store, name)
            flags["store_" + name] = getattr(method, "__func__", method) is not original
    flags.update(judge=loop_judge.llm_stuck_judge is not NATIVE_JUDGE_FUNCTIONS["llm_stuck_judge"],
                 nudge=loop_judge.llm_nudge_writer is not NATIVE_JUDGE_FUNCTIONS["llm_nudge_writer"])
    arguments = {"session_id": session_id, "job_id": job_id, "threshold": threshold,
                 "guard": turn_data(guard) if guard is not None else {}, "digest": digest,
                 "base": os.fspath(getattr(store, "base", "")), "transport": flags,
                 "host": ctx.host.name if ctx is not None else "",
                 "local_floor": getattr(ctx.config, "local_judge_confidence", 0.9) if ctx is not None else 0.9,
                 "synchronous": getattr(ctx, "env", {}).get("TE_JUDGE_SYNC") == "1"}
    runners = {}
    result = invoke(operation, arguments)
    while isinstance(result, dict) and result.get("callback") == "live_judge":
        method, args = result["method"], result["args"]
        try:
            if method.startswith("store_"):
                value = getattr(store, method.removeprefix("store_"))(*args)
                if isinstance(value, os.PathLike):
                    value = os.fspath(value)
            elif method == "paused":
                value = bool(sidecall.paused(ctx.host.name, ctx.root))
            elif method == "runner":
                role, local_only = args
                runners[role] = sidecall.local_runner(role, ctx.config) if local_only else sidecall.runner_for(ctx.host.name, ctx.config, role=role)
                value = runners[role] is not None
            elif method == "run":
                value = runners[args[0]](*args[1:])
            elif method == "estimate":
                value = runners[args[0]].estimate_effective(*args[1:])
            elif method == "local_answer":
                runner = runners[args[0]]
                value = {"answered": bool(getattr(runner, "answered_locally", False)), "local": getattr(runner, "local", "local")}
            elif method == "judge":
                value = loop_judge.llm_stuck_judge(args[0], runners["judge"], model="agent")
            elif method == "nudge":
                value = loop_judge.llm_nudge_writer(args[0], runners["nudge"], model="agent")
            elif method == "spawn_job":
                value = judge.spawn_job(ctx, *args)
            elif method == "spawn":
                value = ctx.spawn(*args)
            else:
                raise AssertionError("Unknown judge callback: " + method)
            reply = {"value": value}
        except Exception as error:
            reply = {"error": str(error), "error_kind": type(error).__name__}
        result = invoke("live_judge.resume", {"job": result["job"], "reply": reply})
    value = storage_result(result)
    return tuple(value) if operation in ("live_judge.poll", "live_judge.no_poll") else value


def live_judge_constructor(self, ctx, session_id, threshold):
    self.ctx, self.sid = ctx, session_id
    self.threshold = live_judge_call("live_judge.new", threshold=threshold)


def live_judge_start(self, guard, digest):
    return live_judge_call("live_judge.start", self.ctx, self.sid, threshold=self.threshold, guard=guard, digest=digest)


def live_judge_poll(self, guard):
    return live_judge_call("live_judge.poll", self.ctx, self.sid, threshold=self.threshold, guard=guard)


def no_verdict_start(self, guard, digest):
    return live_judge_call("live_judge.no_start", guard=guard, digest=digest)


def no_verdict_poll(self, guard):
    return live_judge_call("live_judge.no_poll", guard=guard)


def live_judge_run_job(ctx, session_id, job_id):
    return live_judge_call("live_judge.run_job", ctx, session_id, job_id)


def live_judge_spawn_job(ctx, session_id, job_id):
    return live_judge_call("live_judge.spawn_job", ctx, session_id, job_id)


def spiral_data(state):
    return {name: getattr(state, name) for name in state.__slots__}


def vocabulary_data(vocabulary):
    return {field.name: list(value) if isinstance(value, (tuple, set, frozenset)) else value
            for field in dataclasses.fields(vocabulary) for value in (getattr(vocabulary, field.name),)}


def vocabulary_from_data(data):
    from tokeneconomizer.microskills import GENERIC, NODE_WEB, PYTHON_GIT
    from tokeneconomizer.microskills.observe import Vocabulary
    vocabulary = Vocabulary(**{field.name: field.type(data[field.name]) if field.type in (tuple, frozenset) else data[field.name]
                               for field in dataclasses.fields(Vocabulary)})
    for original in (GENERIC, NODE_WEB, PYTHON_GIT):
        if vocabulary == original:
            return original
    return vocabulary


def live_adapter_data(adapter):
    return dict(vars(adapter), host=adapter.host.name, repo_root=os.fspath(adapter.repo_root), protect=list(adapter.protect))


def live_adapter_constructor(self, host, repo_root, vocabulary="auto", catalog=None, protect=()):
    data = invoke("live_adapter.new", {"host": host.name, "repo_root": os.fspath(repo_root), "vocabulary": vocabulary,
                                      "catalog": dict(catalog or {}), "protect": list(protect)})
    self.__dict__.update(data)
    self.host, self.protect = host, tuple(data["protect"])


def protected_contains(self, name):
    return invoke("live_adapter.protected_contains", {"tools": {"names": list(self), "patterns": list(self.patterns)}, "name": str(name)})


class NativeRegex:
    def __init__(self, pattern):
        self.pattern = pattern

    def search(self, text):
        return invoke("distiller.regex_search", {"pattern": self.pattern, "text": text})


def detector_data(detector):
    return dict(vars(detector), confirm=detector.confirm.pattern if detector.confirm is not None else None,
                contra=detector.contra.pattern if detector.contra is not None else None)


def restore_detector(detector, data):
    detector.__dict__.update(data)
    detector.confirm = NativeRegex(data["confirm"]) if data["confirm"] is not None else None
    detector.contra = NativeRegex(data["contra"]) if data["contra"] is not None else None


def detector_constructor(self, spec):
    restore_detector(self, invoke("detector.new", {"spec": spec}))


def detector_v3_constructor(self, spec, features):
    restore_detector(self, invoke("detector.new", {"spec": spec, "features": features}))


def extractor_constructor(self, spec):
    restore_extractor(self, invoke("extractor.new", {"spec": spec}))


def restore_extractor(extractor, data):
    extractor.__dict__.update(data)
    extractor.rx = NativeRegex(data["rx"]) if data["rx"] is not None else None


def proposed_fact_data(fact):
    return {"spec": fact.spec, "life": vars(fact.life)}


def restore_proposed_fact(fact, data):
    from tokeneconomizer.microskills.lifecycle import Lifecycle
    from tokeneconomizer.microskills.distiller import Detector
    life = getattr(fact, "life", None)
    if life is None:
        life = Lifecycle.__new__(Lifecycle)
    life.__dict__.update(data["life"])
    detector = Detector.__new__(Detector)
    restore_detector(detector, data["detector"])
    fact.__dict__.update(data)
    fact.life = life
    fact.detector = detector


def proposed_fact_constructor(self, spec):
    restore_proposed_fact(self, invoke("proposed_fact.new", {"spec": spec}))


def proposed_fact_from_record(cls, d):
    fact = cls.__new__(cls)
    restore_proposed_fact(fact, invoke("proposed_fact.from_dict", {"d": d}))
    return fact


def distiller_data(distiller):
    data = {key: value for key, value in vars(distiller).items() if key not in ("runner", "external_facts", "facts", "extractors")}
    data["facts"] = {fid: proposed_fact_data(fact) for fid, fact in distiller.facts.items()}
    data["extractors"] = {name: extractor.spec for name, extractor in distiller.extractors.items()}
    return data


def restore_distiller(distiller, data):
    from tokeneconomizer.microskills.distiller import Fact, Extractor
    facts = getattr(distiller, "facts", {})
    restored = {}
    for fid, record in data["facts"].items():
        fact = facts.get(fid)
        if fact is None:
            fact = Fact.__new__(Fact)
        restore_proposed_fact(fact, record)
        restored[fid] = fact
    facts.clear()
    facts.update(restored)
    extractors = {}
    for name, record in data["extractors"].items():
        extractor = Extractor.__new__(Extractor)
        restore_extractor(extractor, record)
        extractors[name] = extractor
    distiller.__dict__.update(data)
    distiller.facts = facts
    distiller.extractors = extractors


def distiller_constructor(self, repo, runner=None, model="agent", external_facts=None, vocab=None):
    self.runner, self.external_facts = runner, external_facts
    restore_distiller(self, invoke("distiller.new", {"repo": repo, "model": model, "generic": getattr(vocab, "generic", False)}))


def distiller_load(cls, repo, root, runner=None, model="agent", external_facts=None, vocab=None):
    distiller = cls.__new__(cls)
    distiller.runner, distiller.external_facts = runner, external_facts
    restore_distiller(distiller, invoke("distiller.load", {"repo": repo, "root": os.fspath(root), "model": model, "generic": getattr(vocab, "generic", False)}))
    return distiller


def lifecycle_from_record(cls, d):
    life = cls.__new__(cls)
    life.__dict__.update(invoke("lifecycle.from_dict", {"d": d}))
    return life


def envfact_data(fact):
    return {"id": fact.id, "family": fact.family, "params": fact.params, "life": vars(fact.life)}


def restore_envfact(fact, data):
    from tokeneconomizer.microskills.lifecycle import Lifecycle
    life = getattr(fact, "life", None)
    if life is None:
        life = Lifecycle.__new__(Lifecycle)
    life.__dict__.update(data["life"])
    fact.__dict__.update(data)
    fact.life = life


def envfact_constructor(self, fid, family, high, params=None):
    restore_envfact(self, invoke("envfact.new", {"fid": fid, "family": family, "high": high, "params": params}))


def refold_targets(targets, env):
    from tokeneconomizer.plugin.context import LiveContext
    from tokeneconomizer.plugin import fold, hosts
    replies = []
    for host, cwd in targets:
        try:
            ctx = LiveContext.build(hosts.HOSTS[host](), cwd, env)
            fold.spawn_fold(ctx, fold.pending_sessions(ctx))
            replies.append({"value": None})
        except Exception:
            replies.append({"error": traceback.format_exc()})
    environment = os.environ if env is None else env
    invoke("repos.finish_refolds", {"replies": replies, "home": os.path.expanduser("~"),
                                    "env": {key: environment[key] for key in ("TOKENECONOMIZER_HOME",) if key in environment}})


def codex_profile_data(profile):
    return dict(vars(profile), path=os.fspath(profile.path))


def restore_codex_profile(profile, data):
    state = getattr(profile, "state", {})
    state.clear()
    state.update(data["state"])
    profile.__dict__.update(data)
    profile.state = state


def codex_profile_constructor(self, repo, results_root, *, fresh=False):
    restore_codex_profile(self, invoke("toolprune.codex.new", {"repo": repo, "results_root": os.fspath(results_root), "fresh": fresh}))


def tooling_data(stage):
    data = {key: value for key, value in vars(stage).items() if key not in ("cfg", "adapter", "judge_tool_runner", "codex_profile")}
    data["cfg"] = dataclasses.asdict(stage.cfg)
    data["codex_profile"] = codex_profile_data(stage.codex_profile) if stage.codex_profile is not None else None
    for name in ("used_tools", "catalog_tools"):
        data[name] = sorted(data[name])
    return data


def restore_tooling(stage, data):
    from tokeneconomizer.config import ToolingConfig
    from tokeneconomizer.toolprune.codex import CodexUntoolProfile
    cfg = getattr(stage, "cfg", None)
    cfg = cfg if cfg is not None else ToolingConfig.__new__(ToolingConfig)
    cfg.__dict__.update(data["cfg"])
    profile = getattr(stage, "codex_profile", None)
    if data["codex_profile"] is not None:
        profile = profile if profile is not None else CodexUntoolProfile.__new__(CodexUntoolProfile)
        restore_codex_profile(profile, data["codex_profile"])
    for name in ("used_tools", "catalog_tools"):
        values = getattr(stage, name, set())
        values.clear()
        values.update(data[name])
        data[name] = values
    stage.__dict__.update(data)
    stage.cfg, stage.codex_profile = cfg, profile


def layers_data(layers):
    from tokeneconomizer.microskills.stage import ComposedDistiller
    from tokeneconomizer.microskills.envfacts import EnvFactsDistiller
    if isinstance(layers, ComposedDistiller):
        return {"env": envfacts_data(layers.env), "dist": distiller_data(layers.dist)}
    if isinstance(layers, EnvFactsDistiller):
        return {"env": envfacts_data(layers), "dist": None}
    return {"env": None, "dist": distiller_data(layers) if layers is not None else None}


def layers_callbacks(layers):
    from tokeneconomizer.microskills.stage import ComposedDistiller
    dist = layers.dist if isinstance(layers, ComposedDistiller) else layers
    return getattr(dist, "runner", None), getattr(dist, "external_facts", None)


def restore_layers(layers, data, runner=None):
    from tokeneconomizer.microskills.stage import ComposedDistiller
    from tokeneconomizer.microskills.envfacts import EnvFactsDistiller
    from tokeneconomizer.microskills.distiller import Distiller
    env = layers.env if isinstance(layers, ComposedDistiller) else layers if isinstance(layers, EnvFactsDistiller) else None
    dist = layers.dist if isinstance(layers, ComposedDistiller) else layers if isinstance(layers, Distiller) else None
    if data["env"] is not None:
        env = env if env is not None else EnvFactsDistiller.__new__(EnvFactsDistiller)
        restore_envfacts(env, data["env"])
    if data["dist"] is not None:
        dist = dist if dist is not None else Distiller.__new__(Distiller)
        dist.runner = getattr(dist, "runner", runner)
        dist.external_facts = getattr(dist, "external_facts", None)
        restore_distiller(dist, data["dist"])
    if data["env"] is not None and data["dist"] is not None:
        composed = layers if isinstance(layers, ComposedDistiller) else ComposedDistiller.__new__(ComposedDistiller)
        composed.env, composed.dist = env, dist
        dist.external_facts = env.injectable_claims
        return composed
    return env if data["env"] is not None else dist


def prefix_data(prefix):
    return {"results_root": os.fspath(prefix.results_root), "reason": prefix.reason, "distiller": layers_data(prefix.distiller)}


def restore_prefix(prefix, data, runner=None):
    prefix.results_root, prefix.reason = data["results_root"], data["reason"]
    prefix.distiller = restore_layers(getattr(prefix, "distiller", None), data["distiller"], runner)


def prefix_constructor(self, cfg, repo, results_root, fresh, runner=None, optimizer_model="agent", adapter=None, agent=None):
    module = importlib.import_module("tokeneconomizer.microskills.distiller")
    vocab = adapter.env_vocabulary() if hasattr(adapter, "env_vocabulary") else None
    result = invoke("prefix.new", {"cfg": dataclasses.asdict(cfg), "repo": repo, "results_root": os.fspath(results_root),
                                  "fresh": fresh, "optimizer_model": optimizer_model,
                                  "vocab": vocabulary_data(vocab) if vocab is not None else None,
                                  "propose_every": module.PROPOSE_EVERY})
    module.PROPOSE_EVERY = result["propose_every"]
    self.adapter, self.agent = adapter, agent
    restore_prefix(self, result["self"], runner)


def restore_manual_verdicts(row):
    if "_task" in row:
        restore_verdicts(row["_task"])


def envfact_from_record(cls, d):
    fact = cls.__new__(cls)
    restore_envfact(fact, invoke("envfact.from_dict", {"d": d}))
    return fact


def envfacts_data(distiller):
    return dict(vars(distiller), facts={fid: envfact_data(fact) for fid, fact in distiller.facts.items()},
                vocab=vocabulary_data(distiller.vocab), last_injected=sorted(distiller.last_injected))


def restore_envfacts(distiller, data):
    from tokeneconomizer.microskills.envfacts import EnvFact
    from tokeneconomizer.microskills.observe import Vocabulary
    facts = getattr(distiller, "facts", {})
    restored = {}
    for fid, record in data["facts"].items():
        fact = facts.get(fid)
        if fact is None:
            fact = EnvFact.__new__(EnvFact)
        restore_envfact(fact, record)
        restored[fid] = fact
    facts.clear()
    facts.update(restored)
    vocabulary = Vocabulary(**{field.name: field.type(data["vocab"][field.name]) if field.type in (tuple, frozenset) else data["vocab"][field.name]
                               for field in dataclasses.fields(Vocabulary)})
    if data["last_digest"] is not None:
        restore_verdicts(data["last_digest"])
    distiller.__dict__.update(data)
    distiller.facts = facts
    distiller.vocab = vocabulary
    distiller.last_injected = set(data["last_injected"])


def restore_verdicts(digest):
    digest["verdicts"] = {fid: tuple(verdict) for fid, verdict in digest["verdicts"].items()}


def envfacts_constructor(self, repo, runner=None, model="agent", vocab=None, max_facts=None, max_tokens=None):
    restore_envfacts(self, invoke("envfacts.new", {"repo": repo, "vocab": vocabulary_data(vocab) if vocab is not None else None,
                                                "max_facts": max_facts, "max_tokens": max_tokens}))


def envfacts_load(cls, repo, root, runner=None, model="agent", vocab=None, max_facts=None, max_tokens=None):
    distiller = cls.__new__(cls)
    restore_envfacts(distiller, invoke("envfacts.load", {"repo": repo, "root": os.fspath(root), "vocab": vocabulary_data(vocab) if vocab is not None else None,
                                                       "max_facts": max_facts, "max_tokens": max_tokens}))
    return distiller


def observation_data(observation):
    return {name: getattr(observation, name) for name in observation.__slots__}


def restore_observation(data):
    from tokeneconomizer.microskills.observe import Observation
    observation = Observation.__new__(Observation)
    for name, value in data.items():
        setattr(observation, name, tuple(value) if name in ("hosts", "labels") else value)
    return observation


def storage_result(result):
    if "raised" in result:
        import builtins
        raise getattr(builtins, result["raised"])(result["message"])
    return result["value"]


def store_constructor(self, base):
    self.__dict__.update(storage_result(invoke("store.new", {"base": os.fspath(base)})))


@contextlib.contextmanager
def store_locked(self, name="state", timeout=10.0):
    lock = storage_result(invoke("store.acquire", {"self": vars(self), "name": name, "timeout": timeout, "async": True}))
    # A blocked native lock must leave the transport available to its holder's thread.
    while True:
        result = storage_result(invoke("store.acquire_result", {"id": lock["id"]}))
        if result["ready"]:
            break
        threading.Event().wait(0.005)
    try:
        yield result["acquired"]
    finally:
        storage_result(invoke("store.release", {"id": lock["id"]}))


def turn_data(guard):
    return dict(vars(guard), st=spiral_data(guard.st), whole=spiral_data(guard.whole))


def restore_turn(guard, data, replace_states=False):
    from tokeneconomizer.loopguard.state import SpiralState
    for key, value in data.items():
        if key in ("st", "whole"):
            state = getattr(guard, key, None)
            if state is None or replace_states:
                state = SpiralState.__new__(SpiralState)
            restore_spiral(state, value)
            value = state
        setattr(guard, key, value)


def turn_constructor(self, turn_key="", data=None):
    restore_turn(self, invoke("turn.new", {"turn_key": turn_key, "data": data}))


class NativeVerifyPattern:
    def search(self, text):
        return invoke("turn.verify_search", {"text": text})


def restore_spiral(state, data):
    for name, value in data.items():
        setattr(state, name, value)


def live_config_constructor(original):
    signature = inspect.signature(original)

    def native(self, *args, **kwargs):
        bound = signature.bind(self, *args, **kwargs)
        fields = dict(bound.arguments)
        fields.pop("self")
        self.__dict__.update(invoke("live_config.new", {"fields": fields}))

    return native


def state_constructor(cls, operation):
    signature = inspect.signature(cls.__init__)

    def native(self, *args, **kwargs):
        bound = signature.bind(self, *args, **kwargs)
        bound.apply_defaults()
        arguments = dict(bound.arguments)
        del arguments["self"]
        self.__dict__.update(invoke(operation, arguments))

    return native


def config_from_args(cls, args):
    from tokeneconomizer.config import InjectionConfig, ToolingConfig, SpiralConfig
    result = invoke("config.from_args", vars(args))
    return cls(injection=InjectionConfig(**result["injection"]),
               tooling=ToolingConfig(**result["tooling"]),
               spiral=SpiralConfig(**result["spiral"]))


def install_opencode_sdk(module):
    fixture = ROOT / "validation/results/opencode-sdk-tests"
    scripts = fixture / "scripts"
    scripts.mkdir(parents=True, exist_ok=True)
    sdk = fixture / "opencode/tokeneconomizer.ts"
    sdk.parent.mkdir(exist_ok=True)
    sdk.write_text((ROOT / "src/plugin/opencode/sdk.ts").read_text(encoding="utf-8"), encoding="utf-8")
    suffix = ".exe" if os.name == "nt" else ""
    shutil.copy2(ROOT / "target/debug" / ("te-repomap-gen" + suffix), scripts / ("te-repomap-gen" + suffix))
    shutil.copy2(ROOT / "target/debug" / ("te" + suffix), scripts / ("te-native" + suffix))
    shutil.copy2(ROOT / "target/debug" / ("te-hook" + suffix), scripts / ("te-hook-native" + suffix))
    shutil.copy2(ROOT / "target/debug" / ("te-console-probe" + suffix), scripts / ("te-hook" + suffix))
    shutil.copy2(ROOT / "target/debug" / ("te-console-probe" + suffix), scripts / ("te" + suffix))
    original_scripts = str(ROOT / "reference/upstream/plugin/scripts")
    original_sdk = str(ROOT / "reference/upstream/plugin/opencode/tokeneconomizer.ts")

    def copytree(source, target, *args, **kwargs):
        return shutil.copytree(scripts if str(source) == original_scripts else source, target, *args, **kwargs)

    def open_sdk(path, *args, **kwargs):
        return builtins.open(sdk if str(path) == original_sdk else path, *args, **kwargs)

    module.shutil = types.SimpleNamespace(**(vars(shutil) | {"copytree": copytree}))
    module.open = open_sdk

    def wrap(original, operation):
        @functools.wraps(original)
        def native(tmp_path, *args, **kwargs):
            original_os = module.os
            temporary = tmp_path / "native-sdk-tmp"
            temporary.mkdir(exist_ok=True)
            environment = dict(os.environ, TMPDIR=str(temporary), TE_MICROSKILLS="envfacts",
                               TE_LOOPGUARD="off", TE_TOOLPRUNE="off", TE_REPOMAP="off",
                               OPENCODE_DISABLE_MODELS_FETCH="1")
            module.os = types.SimpleNamespace(**(vars(os) | {"environ": environment}))
            if operation == "opencode_sdk.runtime_harness" and args and str(args[0]) == original_sdk:
                args = (str(sdk), *args[1:])
            with LOCK:
                CALLS[operation] += 1
            try:
                return original(tmp_path, *args, **kwargs)
            finally:
                module.os = original_os
        return native

    module._run = wrap(module._run, "opencode_sdk.runtime_harness")
    module._service_run = wrap(module._service_run, "opencode_sdk.service")


def pytest_collection_modifyitems(items):
    if "te_harness.guards.net_guard" in sys.modules:
        from native_experiment_net_guard import install
        install()
    if "te_harness.swebench" in sys.modules:
        replacements = install_experiment_harness()
        for module in list(sys.modules.values()):
            if module:
                for name, value in list(vars(module).items()):
                    if inspect.isfunction(value) and value in replacements:
                        setattr(module, name, replacements[value])
    if "te_bench.facade" in sys.modules:
        from native_experiment_facade import install
        install()
    if "te_bench.nudge_guard" in sys.modules:
        from native_experiment_nudge import install
        install()
    if "te_bench.adapters" in sys.modules:
        install_experiment_adapters()
    if "te_bench.codex_spiral" in sys.modules or "te_bench.spiral_stage" in sys.modules:
        from native_experiment_spiral import install
        install()
    module = sys.modules.get("te_bench.baselines.repomap")
    if module is not None:
        module.GEN = str(ROOT / "validation/native_repomap_gen.py")
    if os.environ.get("TE_OPENCODE_E2E") == "1":
        for item in items:
            if str(item.path) == str(ROOT / "reference/upstream/tests/test_opencode_plugin.py") and item.module.OPENCODE:
                install_opencode_sdk(item.module)
                break


def install_experiment_adapters():
    from te_bench.adapters import swebench, webbench, creddata, reposvul
    constants = invoke("experiment.adapters.constants", {})
    for module in (swebench, webbench, creddata, reposvul):
        for name in vars(module).keys() & constants.keys():
            value = constants[name]
            assert value == getattr(module, name) or tuple(value) == getattr(module, name)
            setattr(module, name, tuple(value) if isinstance(value, list) else value)

    def redacted(text):
        return tuple(invoke("experiment.adapters.redact_gold_pointers", {"text": text}))

    def excluded(used_tools=None, extra_inapplicable=()):
        return invoke("experiment.adapters.untool_exclusions", {
            "used_tools": list(used_tools or ()), "extra_inapplicable": list(extra_inapplicable)})

    swebench.redact_gold_pointers = webbench.redact_gold_pointers = redacted
    swebench.untool_exclusions = excluded

    def constructor(original, name):
        signature = inspect.signature(original)
        @functools.wraps(original)
        def native(self, *args, **kwargs):
            values = signature.bind(self, *args, **kwargs)
            values.apply_defaults()
            data = dict(values.arguments)
            data.pop("self")
            for key in ("repo_ids", "instance_ids"):
                if data.get(key) is not None:
                    data[key] = list(data[key])
            data = {key: os.fspath(value) if isinstance(value, os.PathLike) else value for key, value in data.items()}
            vars(self).update(invoke("experiment.adapters.new", {"name": name, "arguments": data, "env": dict(os.environ)}))
        return native

    def method(original, name, operation):
        signature = inspect.signature(original)
        @functools.wraps(original)
        def native(*args, **kwargs):
            values = signature.bind(*args, **kwargs)
            values.apply_defaults()
            data = dict(values.arguments)
            obj = data.pop("self")
            state = {**vars(obj), "name": name} if obj is not None else invoke("experiment.adapters.new", {
                "name": name, "arguments": {"project": ""}, "env": {}})
            if "used_tools" in data:
                data["used_tools"] = list(data["used_tools"] or ())
            data = {key: os.fspath(value) if isinstance(value, os.PathLike) else value for key, value in data.items()}
            state = {key: os.fspath(value) if isinstance(value, os.PathLike) else value for key, value in state.items()}
            result = invoke("experiment.adapters." + operation, {"self": state, "process_cwd": os.getcwd(), **data})
            if "raised" in result:
                if result["raised"] == "JSONDecodeError":
                    raise json.JSONDecodeError(result["message"], "", 0)
                if result["raised"] == "UnicodeDecodeError":
                    raise UnicodeDecodeError("utf-8", bytes(result["bytes"]), result["start"], result["end"], result["message"])
                raise getattr(builtins, result["raised"])(result["message"])
            result = result["value"]
            if operation == "env_vocabulary":
                return vocabulary_from_data(result)
            if operation in ("protected_tools",):
                return frozenset(result)
            if operation in ("inapplicable_tools", "parse_catalog_with_source"):
                return tuple(result)
            if operation == "load_trajectory":
                return [tuple(row) for row in result]
            return result
        return native

    classes = ((swebench.SwebenchAdapter, "swebench"), (webbench.WebBenchAdapter, "webbench"),
               (creddata.CredDataAdapter, "creddata"), (reposvul.ReposVulAdapter, "reposvul"))
    originals = {cls: dict(vars(cls)) for cls, _ in classes}
    for cls, name in classes:
        cls.__init__ = constructor(originals[cls]["__init__"], name)
        for operation in ("load_trajectory", "tool_usage", "parse_catalog", "parse_catalog_with_source", "protected_tools",
                          "inapplicable_tools", "hardcoded_exclusions", "spiral_snapshot", "problem_statements",
                          "env_vocabulary", "classify_domain", "tasks_path", "meta_dir", "repo_ids", "instances_dir", "instance_ids"):
            original = originals[cls].get(operation) or originals[swebench.SwebenchAdapter].get(operation)
            if original is not None:
                setattr(cls, operation, method(original, name, operation))


def pytest_sessionfinish(session, exitstatus):
    path = Path(os.environ.get("TE_NATIVE_COUNTS", ROOT / "validation/results/native-calls.json"))
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(dict(CALLS), indent=2) + "\n", encoding="utf-8")
    if PROCESS is not None:
        PROCESS.stdin.close()
        PROCESS.wait(timeout=10)
        PROCESS.stdout.close()
