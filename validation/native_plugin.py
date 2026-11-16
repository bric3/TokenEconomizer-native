"""Pytest transport adapter: original assertions call the Rust implementation.

This is test compatibility code, retained only until the final test conversion.
Production calculations live in Rust. Original test files remain unmodified.
"""
import collections
import contextlib
import dataclasses
import functools
import importlib
import inspect
import json
import os
from pathlib import Path
import subprocess
import sys
import threading

ROOT = Path(__file__).resolve().parents[1]
CALLS = collections.Counter()
LOCK = threading.Lock()
PROCESS = None
ORIGINAL_REPLACE = os.replace


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
        obj = arguments.get("self")
        if obj is not None:
            if operation.startswith("envfacts."):
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
        result = invoke(operation, arguments)
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
        if operation.startswith(("turn.", "judge.", "store.")) or operation in ("pricing.runner_effective_or_approx", "lifecycle.write_json_atomic", "envelope.load_envelope"):
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
                    else:
                        value = (writer if result["callback"] == "writer" else judge)(result["digest"])
                    replies.append({"value": value, "guard": turn_data(obj) if operation.startswith("turn.") and obj is not None else None})
                except Exception as error:
                    replies.append({"error": str(error), "error_kind": type(error).__name__})
                result = invoke(operation, dict(arguments, replies=replies))
        if operation.startswith("store.") or operation == "lifecycle.write_json_atomic":
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
        "microskills.lifecycle": ("write_json_atomic", "edited_files", "edited_pyfiles"),
        "microskills.observe": ("unwrap", "segments", "_tokens", "repository_top", "_cd_target", "_shell_path", "_shell_isabs",
                                "classify_command", "_runner_script", "_probe_kind", "error_class", "_exit_code_hint", "_label_failed",
                                "_test_labels", "label_rule", "_env_calls", "form_failed", "form_worked", "observe_task", "_error_line",
                                "_base", "_not_found_name", "is_bare"),
        "microskills.envfacts": ("fold", "typed_ok", "_repairs", "_tool_of", "_moves_shell", "_usage", "_root", "_join", "lint_ok"),
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
            operation = operation.removeprefix("loopguard.")
            operation = operation.removeprefix("microskills.")
            replacement = wrap(original, operation)
            replacements[original] = replacement
            setattr(module, name, replacement)

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


def spiral_data(state):
    return {name: getattr(state, name) for name in state.__slots__}


def vocabulary_data(vocabulary):
    return {field.name: list(value) if isinstance(value, (tuple, set, frozenset)) else value
            for field in dataclasses.fields(vocabulary) for value in (getattr(vocabulary, field.name),)}


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
