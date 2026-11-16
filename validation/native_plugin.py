"""Pytest transport adapter: original assertions call the Rust implementation.

This is test compatibility code, retained only until the final test conversion.
Production calculations live in Rust. Original test files remain unmodified.
"""
import collections
from collections.abc import Iterator
import contextlib
import dataclasses
import functools
import importlib
import inspect
import json
import math
import os
from pathlib import Path
import subprocess
import sys
import threading
import traceback

ROOT = Path(__file__).resolve().parents[1]
CALLS = collections.Counter()
LOCK = threading.Lock()
PROCESS = None
ORIGINAL_REPLACE = os.replace
LIVE_CONFIG_ENV = []
NATIVE_STORE_WRITE = None
NATIVE_DENY_FUNCTIONS = {}
NATIVE_JUDGE_FUNCTIONS = {}
NATIVE_JUDGE_STORE = {}
NATIVE_QUEUE_ENQUEUE = None
NATIVE_HISTORY_FUNCTIONS = {}


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
        if operation.startswith("hookqueue."):
            return hookqueue_call(operation, arguments)
        if operation.startswith("history."):
            return history_call(operation, arguments)
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
    sys.path.insert(0, str(ROOT / "reference/upstream"))
    groups = {
        "pricing": ("resolve_model", "weights_for", "has_breakdown", "cache_write_1h_tokens",
                    "effective_tokens", "approx_tokens", "runner_effective_or_approx"),
        "ledger": ("overhead", "effective_exec"),
        "plugin.accounting": ("effective", "overhead_total", "summarize", "fmt_tokens"),
        "plugin.config": ("canonical", "_threshold_name", "_read", "_old_names_first", "load_config", "_drop_old_names", "write_config", "unset_config", "_write"),
        "plugin.adapter": ("detect_vocabulary", "catalog_path", "load_catalog"),
        "plugin.hookqueue": ("_safe", "_queues", "enqueue", "drain", "_replay", "_prune", "run"),
        "plugin.history": ("_record_error", "read_state", "_num", "_stale", "_ledger_size", "_session_records", "_nothing_folded", "_live_folds", "_folded_by_import", "_refusal", "_identity", "is_this_repository", "_headless", "plan", "_still_eligible", "replay_envelope", "_update", "_save", "_say", "run", "_run_locked", "_import_one", "maybe_start", "notice", "summary", "describe"),
        "plugin.hosts": ("claude_success_code",),
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
                operation = "host_helpers." + name
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
    global LIVE_CONFIG_ENV
    LIVE_CONFIG_ENV = invoke("live_config.environment", {})
    from tokeneconomizer.plugin import repos, hosts
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


def history_call(operation, arguments):
    from tokeneconomizer.plugin import history, fold
    arguments = dict(arguments)
    ctx = arguments.pop("ctx", None)
    store = arguments.pop("store", getattr(ctx, "store", None))
    log, change = arguments.pop("log", None), arguments.pop("change", None)
    original_doc, original_skipped, original_cache = arguments.get("doc"), arguments.get("skipped"), arguments.get("cache")
    aliased_skipped = isinstance(original_doc, dict) and original_doc.get("skipped") is original_skipped and original_skipped is not None
    arguments["ctx"] = {"root": os.fspath(getattr(ctx, "root", ".")), "base": os.fspath(getattr(store, "base", ".")),
                        "host": getattr(getattr(ctx, "host", None), "name", "claude"),
                        "keeps_history": bool(getattr(getattr(ctx, "host", None), "keeps_history", False)),
                        "config": dataclasses.asdict(ctx.config) if ctx is not None and hasattr(ctx, "config") else {},
                        "env": dict(getattr(ctx, "env", {})), "home": os.path.expanduser("~")}
    if ctx is not None and hasattr(ctx, "repo"):
        arguments["ctx"]["repo"] = dataclasses.asdict(ctx.repo)
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


def pytest_sessionfinish(session, exitstatus):
    path = Path(os.environ.get("TE_NATIVE_COUNTS", ROOT / "validation/results/native-calls.json"))
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(dict(CALLS), indent=2) + "\n", encoding="utf-8")
    if PROCESS is not None:
        PROCESS.stdin.close()
        PROCESS.wait(timeout=10)
        PROCESS.stdout.close()
