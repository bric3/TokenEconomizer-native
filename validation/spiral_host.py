"""JSON I/O and model/adapter stand-ins for source/native spiral comparisons."""
import contextlib
import dataclasses
import io
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT / "reference/upstream"), str(ROOT / "reference/upstream/experiments")]
from te_bench import codex_spiral as cs
from te_bench.spiral_stage import SpiralStage
from tokeneconomizer.config import SpiralConfig
from native_experiment_spiral import state_data

if "--native" in sys.argv:
    from native_experiment_spiral import install
    install()


def data(value):
    if dataclasses.is_dataclass(value):
        return data(dataclasses.asdict(value))
    if isinstance(value, cs.CodexSpiralState):
        return data(state_data(value))
    if isinstance(value, (set, frozenset)):
        return sorted(data(v) for v in value)
    if isinstance(value, (list, tuple)):
        return [data(v) for v in value]
    if isinstance(value, dict):
        return {key: data(v) for key, v in value.items()}
    if isinstance(value, float) and value.is_integer():
        return int(value)
    if callable(value):
        return "callable"
    return value


for line in sys.stdin:
    request = json.loads(line)
    messages = io.StringIO()
    try:
        with contextlib.redirect_stdout(messages):
            kind = request.get("kind", "function")
            if kind == "function":
                value = getattr(cs, request["method"])(**request.get("arguments", {}))
            elif kind == "replay":
                policy = cs.CodexSpiralPolicy(**request.get("policy", {}))
                first, state = cs.replay_events(cs.iter_events(request["lines"], first_thread_only=request.get("first_thread_only", False)), policy)
                guard = cs.CodexResumeGuard(policy, verbose=request.get("verbose", False))
                stops = []
                for entry in request["lines"]:
                    fired = guard.monitor(entry)
                    resume = guard.continuation() if fired and request.get("continue", True) else None
                    stops.append({"fired": fired, "resume": resume})
                value = {"decision": first, "state": state, "stops": stops, "telemetry": guard.telemetry(), "pending": guard.pending_reason, "segment": guard.segment}
            elif kind == "state":
                state = cs.CodexSpiralState(request.get("recent_window", 12))
                policy = cs.CodexSpiralPolicy(**request.get("policy", {}))
                snapshots = []
                for entry in request["events"]:
                    state.update(cs.CodexEvent(**dict(entry, paths=tuple(entry.get("paths", [])))))
                    snapshots.append({"snapshot": state.snapshot(), "decision": state.decision(policy)})
                value = {"state": state, "steps": snapshots}
            elif kind == "tracker":
                tracker = cs.CodexEnvelopeTracker(**request.get("arguments", {}))
                steps = [tracker.snapshot()]
                for turns, resolved in request["rows"]:
                    tracker.update(turns, resolved)
                    steps.append(tracker.snapshot())
                value = {"steps": steps, "state": vars(tracker)}
            elif kind == "stage":
                calls = []
                class Adapter:
                    def spiral_snapshot(self, path):
                        calls.append({"snapshot": path})
                        if request.get("snapshot_error"):
                            raise OSError("fixture snapshot error")
                        return request.get("snapshot", {})
                class Runner:
                    def __call__(self, prompt, model="agent"):
                        calls.append({"prompt": prompt, "model": model})
                        if request.get("runner_error"):
                            raise RuntimeError("fixture model error")
                        return request.get("reply", "{}"), request.get("tokens", 3.5)
                    def estimate_effective(self, prompt, raw):
                        calls.append({"estimate": [prompt, raw]})
                        return 9.0
                cfg = SpiralConfig(**request.get("config", {}))
                stage = SpiralStage(cfg, request.get("max_turns", 80), runner=Runner() if request.get("runner") else None,
                                    optimizer_model=request.get("model", "agent"), adapter=Adapter() if request.get("adapter") else None,
                                    agent=request.get("agent"))
                for key, value in request.get("mutate", {}).items():
                    setattr(cfg, key, value)
                rows = []
                for row in request.get("rows", []):
                    summary = dict(request.get("summary", {}))
                    before = stage.run_kwargs({})
                    result = stage.after_task({}, row, request.get("run_dir", "run"), summary)
                    rows.append({"before": before, "result": result, "summary": summary})
                judge = stage.spiral_judge(request["digest"]) if "digest" in request and stage.spiral_judge else None
                nudge = stage.spiral_nudge_writer(request["digest"]) if "digest" in request and stage.spiral_nudge_writer else None
                value = {"rows": rows, "kwargs": stage.run_kwargs({}), "envelope": vars(stage.envelope),
                         "tracker": vars(stage.codex_tracker) if stage.codex_tracker else None, "judge": judge, "nudge": nudge, "calls": calls}
            elif kind == "iterator":
                reads = []
                def lines():
                    for index, value in enumerate(request["lines"]):
                        reads.append(index)
                        yield value
                events = cs.iter_events(lines(), first_thread_only=request.get("first_thread_only", False))
                before = list(reads)
                values = []
                for _ in range(request.get("take", 100)):
                    try:
                        values.append(next(events))
                    except StopIteration:
                        break
                value = {"before": before, "reads": reads, "events": values}
            else:
                raise AssertionError(kind)
        response = {"value": data(value), "messages": messages.getvalue()}
    except Exception as error:
        response = {"raised": type(error).__name__, "message": str(error), "messages": messages.getvalue()}
    print(json.dumps(response, sort_keys=True, separators=(",", ":"), ensure_ascii=False), flush=True)
