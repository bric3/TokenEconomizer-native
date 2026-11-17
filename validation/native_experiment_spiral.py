"""Original experiment spiral classes as JSON bindings; all policy executes in Rust."""
import builtins
import collections
import dataclasses
import functools
import inspect
import os

from native_plugin import invoke


def checked(value):
    if isinstance(value, dict) and "raised" in value:
        raise getattr(builtins, value["raised"])(value["message"])
    return value


def state_data(obj):
    return dict(vars(obj), edited_paths=dict(obj.edited_paths), failure_sigs_since_green=dict(obj.failure_sigs_since_green),
                recent_commands=list(obj.recent_commands), recent_blocked=list(obj.recent_blocked),
                recent_window=obj.recent_commands.maxlen, outstanding_failure_scopes=list(obj.outstanding_failure_scopes))


def restore_state(obj, data):
    for key, value in data.items():
        if key == "recent_window":
            continue
        if key in ("edited_paths", "failure_sigs_since_green"):
            value = collections.Counter(value)
        elif key in ("recent_commands", "recent_blocked"):
            value = collections.deque(value, maxlen=data["recent_window"])
        elif key == "outstanding_failure_scopes":
            value = set(value)
        setattr(obj, key, value)
    return obj


def guard_data(obj):
    return dict(vars(obj), state=state_data(obj.state), policy=dataclasses.asdict(obj.policy), fired_reasons=list(obj.fired_reasons))


def constructor(original, operation, restore):
    signature = inspect.signature(original)
    @functools.wraps(original)
    def native(self, *args, **kwargs):
        bound = signature.bind(self, *args, **kwargs)
        bound.apply_defaults()
        data = dict(bound.arguments)
        data.pop("self")
        if "policy" in data:
            data["policy"] = dataclasses.asdict(data["policy"])
        if operation == "tracker":
            data = {"fields": data}
        restore(self, checked(invoke("codex_spiral." + operation + ".new", data)))
    return native


def method(original, operation, encode, restore):
    signature = inspect.signature(original)
    @functools.wraps(original)
    def native(*args, **kwargs):
        bound = signature.bind(*args, **kwargs)
        bound.apply_defaults()
        data = dict(bound.arguments)
        obj = data.pop("self")
        for key in ("event", "policy"):
            if key in data:
                data[key] = dataclasses.asdict(data[key])
        result = checked(invoke("codex_spiral." + operation, {"self": encode(obj), **data}))
        restore(obj, result["self"])
        for message in result.get("messages", []):
            print(message)
        return result["value"]
    return native


def stage_data(obj):
    return {"cfg": dataclasses.asdict(obj.cfg), "max_turns": obj.max_turns, "optimizer_model": obj._native_optimizer_model,
            "agent": obj.agent, "envelope": vars(obj.envelope), "spiral_judge": obj.spiral_judge is not None,
            "spiral_nudge_writer": obj.spiral_nudge_writer is not None,
            "codex_tracker": vars(obj.codex_tracker) if obj.codex_tracker is not None else None}


def restore_stage(obj, data):
    from te_bench.codex_spiral import CodexEnvelopeTracker
    from tokeneconomizer.loopguard import EnvelopeTracker
    obj.max_turns, obj.agent = data["max_turns"], data["agent"]
    obj._native_optimizer_model = data["optimizer_model"]
    if not hasattr(obj, "envelope"):
        obj.envelope = EnvelopeTracker.__new__(EnvelopeTracker)
    vars(obj.envelope).update(data["envelope"])
    if data["codex_tracker"] is not None:
        if getattr(obj, "codex_tracker", None) is None:
            obj.codex_tracker = CodexEnvelopeTracker.__new__(CodexEnvelopeTracker)
        vars(obj.codex_tracker).update(data["codex_tracker"])
    else:
        obj.codex_tracker = None


def stage_call(obj, name, data):
    replies = []
    while True:
        result = checked(invoke("spiral_stage." + name, dict(data, replies=replies)))
        if "callback" not in result:
            return result
        try:
            if result["callback"] == "snapshot":
                value = obj.adapter.spiral_snapshot(result["path"])
            elif result["callback"] == "runner":
                value = obj._native_runner(result["prompt"], result["model"])
            elif result["callback"] == "estimate":
                value = obj._native_runner.estimate_effective(result["prompt"], result["raw"])
            replies.append({"value": value})
        except Exception as error:
            replies.append({"error": str(error)})


def stage_constructor(self, cfg, max_turns, runner=None, optimizer_model="agent", adapter=None, agent=None):
    self.cfg, self.adapter, self._native_runner = cfg, adapter, runner
    result = stage_call(self, "new", {"cfg": dataclasses.asdict(cfg), "max_turns": max_turns, "optimizer_model": optimizer_model, "agent": agent})
    data = result["self"]
    restore_stage(self, data)
    self.spiral_judge = (lambda digest: stage_invoke(self, "judge", {"digest": digest})) if data["spiral_judge"] else None
    self.spiral_nudge_writer = (lambda digest: stage_invoke(self, "nudge_writer", {"digest": digest})) if data["spiral_nudge_writer"] else None


def stage_invoke(self, name, data):
    result = stage_call(self, name, {"self": stage_data(self), "runner": self._native_runner is not None,
                                   "adapter": self.adapter is not None, **data})
    restore_stage(self, result["self"])
    return checked(result["value"])


def stage_kwargs(self, task):
    from te_bench.codex_spiral import CodexSpiralPolicy
    value = stage_invoke(self, "run_kwargs", {})
    if "spiral_judge" in value:
        value["spiral_judge"] = self.spiral_judge
        value["spiral_nudge_writer"] = self.spiral_nudge_writer
    if value.get("codex_spiral_policy") is not None:
        value["codex_spiral_policy"] = CodexSpiralPolicy(**value["codex_spiral_policy"])
    return value


def stage_after(self, task, row, run_dir_path, srow):
    result = stage_call(self, "after_task", {"self": stage_data(self), "row": row, "srow": srow,
                                           "run_dir_path": os.fspath(run_dir_path), "adapter": self.adapter is not None})
    restore_stage(self, result["self"])
    srow.clear()
    srow.update(result["srow"])
    return checked(result["value"])


def install():
    from te_bench import codex_spiral as cs
    from te_bench.spiral_stage import SpiralStage
    constants = invoke("codex_spiral.constants", {})
    for name in ("PROFILE_VERSION", "DEFENSIVE_PROFILE_VERSION", "CODEX_DEFAULT_PROFILE"):
        assert constants[name] == getattr(cs, name)
        setattr(cs, name, constants[name])
    for name in ("DEFAULT_POLICY", "SHIPPED_POLICY", "DEFENSIVE_POLICY"):
        assert constants[name] == dataclasses.asdict(getattr(cs, name))
        setattr(cs, name, cs.CodexSpiralPolicy(**constants[name]))

    def event(data):
        return cs.CodexEvent(**dict(data, paths=tuple(data["paths"])))

    def restore_guard(obj, data):
        obj.policy = cs.CodexSpiralPolicy(**data["policy"])
        obj.state = restore_state(getattr(obj, "state", cs.CodexSpiralState.__new__(cs.CodexSpiralState)), data["state"])
        for key in ("verbose", "nudges_done", "pending_reason", "events"):
            setattr(obj, key, data[key])
        obj.fired_reasons = set(data["fired_reasons"])

    cs.CodexSpiralState.__init__ = constructor(cs.CodexSpiralState.__init__, "state", restore_state)
    for name in ("update", "decision", "snapshot"):
        setattr(cs.CodexSpiralState, name, method(getattr(cs.CodexSpiralState, name), "state." + name, state_data, restore_state))
    for name in ("distinct_edit_files", "edit_churn", "green_after_edit", "currently_green", "repeated_failure_count"):
        setattr(cs.CodexSpiralState, name, property(method(getattr(cs.CodexSpiralState, name).fget, "state." + name, state_data, restore_state)))
    cs.CodexResumeGuard.__init__ = constructor(cs.CodexResumeGuard.__init__, "guard", restore_guard)
    for name in ("monitor", "continuation", "telemetry"):
        setattr(cs.CodexResumeGuard, name, method(getattr(cs.CodexResumeGuard, name), "guard." + name, guard_data, restore_guard))
    cs.CodexResumeGuard.segment = property(method(cs.CodexResumeGuard.segment.fget, "guard.segment", guard_data, restore_guard))
    restore_tracker = lambda obj, data: vars(obj).update(data)
    cs.CodexEnvelopeTracker.__init__ = constructor(cs.CodexEnvelopeTracker.__init__, "tracker", restore_tracker)
    for name in ("update", "snapshot"):
        setattr(cs.CodexEnvelopeTracker, name, method(getattr(cs.CodexEnvelopeTracker, name), "tracker." + name, vars, restore_tracker))
    cs.CodexEnvelopeTracker.threshold = property(method(cs.CodexEnvelopeTracker.threshold.fget, "tracker.threshold", vars, restore_tracker))

    def function(original, name):
        signature = inspect.signature(original)
        @functools.wraps(original)
        def native(*args, **kwargs):
            bound = signature.bind(*args, **kwargs)
            bound.apply_defaults()
            data = dict(bound.arguments)
            if name == "policy.at_threshold":
                data["self"] = dataclasses.asdict(data["self"])
            if name == "replay_events":
                data["events"] = [dataclasses.asdict(ev) for ev in data["events"]]
                data["policy"] = dataclasses.asdict(data["policy"])
            value = checked(invoke("codex_spiral." + name, data))
            if name in ("shipped_policy", "defensive_policy", "profile_policy", "policy.at_threshold"):
                return cs.CodexSpiralPolicy(**value)
            if name in ("verification_scopes", "failure_signatures"):
                return set(value) if value is not None else None
            if name == "patch_paths":
                return tuple(value)
            if name == "parse_events":
                return [event(ev) for ev in value]
            if name == "replay_events":
                return value["decision"], restore_state(cs.CodexSpiralState.__new__(cs.CodexSpiralState), value["state"])
            return value
        return native

    cs.CodexSpiralPolicy.at_threshold = function(cs.CodexSpiralPolicy.at_threshold, "policy.at_threshold")
    for name in ("shipped_policy", "defensive_policy", "profile_policy", "profile_version", "_normalize", "_command_fingerprint",
                 "green_is_trustworthy", "verification_scopes", "is_test_command", "is_verification_command", "is_research_command",
                 "failure_signatures", "patch_paths", "shell_patch_applied", "parse_events", "replay_events", "_percentile", "nudge_text"):
        setattr(cs, name, function(getattr(cs, name), name))

    def iter_events(lines, *, first_thread_only=False):
        cursor = None
        for line in lines:
            value = checked(invoke("codex_spiral.iterator", {"self": cursor, "line": line, "first_thread_only": first_thread_only}))
            cursor = value["self"]
            yield from (event(ev) for ev in value["events"])
            if cursor["finished"]:
                return
    cs.iter_events = iter_events
    SpiralStage.__init__, SpiralStage.run_kwargs, SpiralStage.after_task = stage_constructor, stage_kwargs, stage_after
