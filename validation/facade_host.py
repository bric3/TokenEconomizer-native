"""JSON I/O, fixture files and external stand-ins for facade comparisons."""
import contextlib
import dataclasses
import io
import json
import sys
import types
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT / "reference/upstream"), str(ROOT / "reference/upstream/experiments")]
from te_bench.facade import Economizer
from te_bench import runner as runner_module
from tokeneconomizer.config import EconomizerConfig, InjectionConfig, ToolingConfig, SpiralConfig
from tokeneconomizer.microskills import PYTHON_GIT, NODE_WEB, distiller
from native_plugin import prefix_data, tooling_data

if "--native" in sys.argv:
    from native_experiment_facade import install
    install()


def data(value):
    if dataclasses.is_dataclass(value):
        return data(dataclasses.asdict(value))
    if isinstance(value, dict):
        return {key: data(item) for key, item in value.items()}
    if isinstance(value, (set, frozenset)):
        return sorted(data(item) for item in value)
    if isinstance(value, (list, tuple)):
        return [data(item) for item in value]
    if isinstance(value, float) and value.is_integer():
        return int(value)
    if callable(value):
        return "callable"
    return value


def snapshot(econ):
    spiral = econ.spiral
    injection = prefix_data(econ.injection)
    env = injection["distiller"]["env"]
    if env is not None:
        env["vocab"] = {key: sorted(value) if isinstance(value, list) and key != "runner_scripts" else value for key, value in env["vocab"].items()}
    return data({"config": econ.config, "injection": injection, "tooling": tooling_data(econ.tooling),
                 "spiral": {"cfg": spiral.cfg, "max_turns": spiral.max_turns, "agent": spiral.agent,
                            "envelope": vars(spiral.envelope), "codex_tracker": vars(spiral.codex_tracker) if spiral.codex_tracker else None,
                            "judge": spiral.spiral_judge, "writer": spiral.spiral_nudge_writer},
                 "adapter": type(econ.adapter).__name__, "propose_every": distiller.PROPOSE_EVERY})


for line in sys.stdin:
    request = json.loads(line)
    messages = io.StringIO()
    calls = []
    factory_index = [0]
    def answer(kind, prompt, model):
        calls.append({"runner": kind, "prompt": prompt, "model": model})
        if request.get("model_error"):
            raise RuntimeError("fixture model error")
        return request.get("reply", '{"facts":[],"extractors":[],"stuck":false,"text":"Try a focused check."}'), request.get("tokens", 3.5)
    class ProtocolRunner:
        def optimizer_call(self, prompt, model="agent"):
            return answer("optimizer", prompt, model)
        def __repr__(self):
            raise AssertionError("valid runner representation must stay lazy")
    class BothRunner(ProtocolRunner):
        def __call__(self, prompt, model="agent"):
            return answer("call", prompt, model)
    class InvalidRunner:
        def __repr__(self):
            return "InvalidRunner()"
    def make_candidate(kind):
        if kind == "protocol": return ProtocolRunner()
        if kind == "both": return BothRunner()
        if kind == "invalid": return InvalidRunner()
        if kind == "false": return False
        if kind == "none": return None
        return lambda prompt, model="agent": answer("call", prompt, model)
    class Factory:
        def __bool__(self):
            calls.append({"factory_bool": True, "propose_every": distiller.PROPOSE_EVERY})
            return not request.get("false_factory", False)
        def __call__(self):
            calls.append({"factory": factory_index[0], "propose_every": distiller.PROPOSE_EVERY})
            if request.get("factory_error"):
                raise RuntimeError("fixture factory error")
            kinds = request.get("factory_kinds", ["callable"])
            kind = kinds[factory_index[0] % len(kinds)]
            factory_index[0] += 1
            return make_candidate(kind)
    class FixtureAdapter:
        def env_vocabulary(self):
            calls.append({"adapter": "env_vocabulary", "propose_every": distiller.PROPOSE_EVERY})
            return NODE_WEB if request.get("vocabulary") == "node" else PYTHON_GIT
        def problem_statements(self):
            calls.append({"adapter": "problem_statements"})
            if request.get("problem_error"): raise OSError("fixture problem error")
            return request.get("problems", {"task": "Fix this task"})
        def protected_tools(self):
            calls.append({"adapter": "protected_tools"})
            return frozenset(request.get("protected", ["bash"]))
        def tool_usage(self, path):
            calls.append({"adapter": "tool_usage", "path": str(path)})
            if request.get("usage_error"): raise OSError("fixture usage error")
            return request.get("usage", {"bash": 1})
        def parse_catalog_with_source(self, path, agent):
            calls.append({"adapter": "catalog", "path": str(path), "agent": agent})
            if request.get("catalog_error"): raise OSError("fixture catalog error")
            return request.get("catalog", {"bash": "shell", "web": "network"}), "fixture"
        def spiral_snapshot(self, path):
            calls.append({"adapter": "spiral_snapshot", "path": str(path)})
            if request.get("snapshot_error"): raise OSError("fixture snapshot error")
            return request.get("snapshot", {})
    try:
        with contextlib.redirect_stdout(messages):
            distiller.PROPOSE_EVERY = 8
            root = Path(request["root"])
            root.mkdir(parents=True, exist_ok=True)
            for name, content in request.get("files", {}).items():
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content, encoding="utf-8")
            candidate = make_candidate(request.get("runner", "none"))
            if request.get("kind") == "optimizer":
                adapted = runner_module.as_optimizer_callable(candidate)
                value = {"none": adapted is None, "identity": adapted is candidate,
                         "answer": adapted("prompt", "model") if adapted is not None else None}
            else:
                config = request.get("config", {})
                cfg = EconomizerConfig(InjectionConfig(**config.get("injection", {})), ToolingConfig(**config.get("tooling", {})), SpiralConfig(**config.get("spiral", {})))
                common = {"repo": "acme/widget", "results_root": str(root),
                          "runner": candidate, "make_runner": Factory() if request.get("factory") else None,
                          "adapter": FixtureAdapter() if request.get("adapter", True) else None}
                def construct(fresh):
                    if request.get("kind") == "from_args":
                        return Economizer.from_args(types.SimpleNamespace(**request.get("arguments", {"agent": "claude"})), fresh=fresh, **common)
                    return Economizer(cfg, agent=request.get("agent", "claude"), optimizer_model=request.get("model", "agent"),
                                      max_turns=request.get("max_turns", 80), fresh=fresh, **common)
                econ = construct(request.get("fresh", True))
                steps = []
                for operation in request.get("operations", []):
                    for stage, fields in operation.get("config", {}).items():
                        for key, field in fields.items():
                            setattr(getattr(econ.config, stage), key, field)
                    arguments = dict(operation.get("arguments", {}))
                    try:
                        target = econ.spiral if operation.get("target") == "spiral" else econ
                        result = getattr(target, operation["method"])(**arguments)
                        steps.append({"value": data(result), "summary": arguments.get("srow"), "state": snapshot(econ)})
                    except Exception as error:
                        steps.append({"raised": type(error).__name__, "message": str(error), "summary": arguments.get("srow"), "state": snapshot(econ)})
                saved = {str(path.relative_to(root)): json.loads(path.read_text(encoding="utf-8")) for path in sorted(root.rglob("*.json"))}
                value = {"steps": steps, "state": snapshot(econ), "files": data(saved)}
                if request.get("reload"):
                    value["reload"] = snapshot(construct(False))
        response = {"value": data(value), "calls": calls, "messages": messages.getvalue(), "propose_every": distiller.PROPOSE_EVERY}
    except Exception as error:
        response = {"raised": type(error).__name__, "message": str(error), "calls": calls, "messages": messages.getvalue(), "propose_every": distiller.PROPOSE_EVERY}
    print(json.dumps(response, sort_keys=True, separators=(",", ":"), ensure_ascii=False), flush=True)
