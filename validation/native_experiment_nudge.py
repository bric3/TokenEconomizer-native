"""Data and callback transport for the original experiment controller tests."""
import functools
import inspect
import json

from native_plugin import invoke, spiral_data, restore_spiral
from native_experiment_spiral import checked


def guard_data(obj):
    return {**{key: value for key, value in vars(obj).items() if key not in ("judge", "nudge_writer", "st", "pend")},
            "st": spiral_data(obj.st),
            "pend": {json.dumps(key, ensure_ascii=False, separators=(",", ":")): value for key, value in obj.pend.items()}}


def restore(obj, data):
    from tokeneconomizer.loopguard.state import SpiralState
    for key, value in data.items():
        if key == "st":
            state = SpiralState.__new__(SpiralState)
            restore_spiral(state, value)
            value = state
        elif key == "pend":
            value = {json.loads(key): item for key, item in value.items()}
        setattr(obj, key, value)


def install():
    from te_bench import nudge_guard as module
    constants = invoke("nudge_guard.constants", {})
    for name in ("RESUME_HEADER", "DEFAULT_THRESHOLDS"):
        assert constants[name] == getattr(module, name)
        setattr(module, name, constants[name])
    cls = module.NudgeGuard
    original = cls.__init__

    @functools.wraps(original)
    def constructor(self, envelope=None, judge=None, thr=None, nudge_budget=1, verbose=False, nudge_writer=None, resume_header=None):
        self.judge, self.nudge_writer = judge, nudge_writer
        restore(self, checked(invoke("nudge_guard.new", {"envelope": envelope, "thr": thr, "nudge_budget": nudge_budget,
                    "verbose": verbose, "resume_header": resume_header})))
    cls.__init__ = constructor

    def method(original, name):
        signature = inspect.signature(original)
        @functools.wraps(original)
        def native(self, *args, **kwargs):
            bound = signature.bind(self, *args, **kwargs)
            bound.apply_defaults()
            arguments = dict(bound.arguments)
            arguments["self"] = guard_data(self)
            arguments["judge"] = bool(self.judge) if name == "monitor" else self.judge is not None
            arguments["writer"] = self.nudge_writer is not None
            replies = []
            while True:
                result = checked(invoke("nudge_guard." + name, dict(arguments, replies=replies)))
                if "callback" not in result:
                    restore(self, result["self"])
                    for message in result["messages"]:
                        print(message)
                    return checked(result["value"])
                try:
                    callback = self.judge if result["callback"] == "judge" else self.nudge_writer
                    replies.append({"value": callback(result["digest"])})
                except Exception as error:
                    replies.append({"error": str(error)})
        return native

    for name in ("monitor", "_sampled_judge", "_nudge_body", "continuation", "_stop"):
        setattr(cls, name, method(getattr(cls, name), name))
