"""Pytest transport adapter: original assertions call the Rust implementation.

This is test compatibility code, retained only until the final test conversion.
Production calculations live in Rust. Original test files remain unmodified.
"""
import collections
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
        arguments = {name: os.fspath(value) if isinstance(value, os.PathLike) else value
                     for name, value in arguments.items()}
        for name in ("lines", "turn_lines"):
            if name in arguments:
                arguments[name] = list(arguments[name])
        if operation == "config.needs_optimizer":
            arguments["config"] = dataclasses.asdict(arguments["config"])
        obj = arguments.get("self")
        if obj is not None:
            arguments["self"] = vars(obj) if operation.startswith("envelope.") else obj.to_dict()
        if "tracker" in arguments:
            arguments["tracker"] = vars(arguments["tracker"])
        state = arguments.get("state")
        if operation.startswith("rules.") and state is not None:
            arguments["state"] = {name: getattr(state, name) for name in (
                "turns", "edit_churn", "distinct_edit_files", "error_outputs", "stagnation",
                "repeated_fail_sigs", "test_runs", "edit_counts")}
        judge = arguments.get("judge")
        if operation == "rules.assess_abort":
            arguments["judge"] = judge is not None
        if "files" in arguments:
            arguments["files"] = sorted(arguments["files"])
        result = invoke(operation, arguments)
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
        if operation == "trace.claude.claude_calls":
            return [tuple(call) for call in result]
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
                    "effective_tokens", "approx_tokens"),
        "ledger": ("overhead", "effective_exec"),
        "plugin.accounting": ("effective", "overhead_total", "summarize", "fmt_tokens"),
        "names": ("repo_filename",),
        "config": ("needs_optimizer",),
        "loopguard.rules": ("scaled_thresholds", "gate_for_judge", "is_stuck", "nudge_text", "assess_abort"),
        "loopguard.envelope": ("_percentile", "thresholds", "base_thresholds"),
        "trace.jsonl": ("lines", "records"),
        "trace.claude": ("claude_calls", "_is_prompt", "_is_notification",
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
            replacement = wrap(original, operation)
            replacements[original] = replacement
            setattr(module, name, replacement)

    from tokeneconomizer.microskills.lifecycle import Lifecycle
    for name in ("_drifted_confirms", "score", "firing_rate", "injectable", "update"):
        setattr(Lifecycle, name, wrap(getattr(Lifecycle, name), "lifecycle." + name))

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
