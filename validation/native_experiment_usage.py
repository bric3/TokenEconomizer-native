"""Original harness functions as data-only bindings to native usage accounting."""
import functools
import inspect
import os

from native_plugin import invoke
from native_experiment_spiral import checked


def install():
    from te_harness import swebench, codex, copilot
    replacements = {}
    def function(original, operation):
        signature = inspect.signature(original)
        @functools.wraps(original)
        def native(*args, **kwargs):
            bound = signature.bind(*args, **kwargs)
            bound.apply_defaults()
            arguments = {key: os.fspath(value) if isinstance(value, os.PathLike) else value for key, value in bound.arguments.items()}
            if "log_dir" in arguments:
                arguments["process_cwd"] = os.getcwd()
            result = checked(invoke("experiment.usage." + operation, arguments))
            if operation == "claude.session_output_tokens":
                return result[0], set(result[1])
            return result
        return native
    for module, domain, names in ((swebench,"claude",("_merge_usage", "session_output_tokens", "_row_from_result")),
                                  (codex,"codex",("row_usage", "merge_usage")),
                                  (copilot,"copilot",("sum_openai_usage", "read_log_dir", "turns_from_stream", "stream_output_tokens", "row_usage", "merge_usage"))):
        for name in names:
            original = getattr(module, name)
            replacements[original] = function(original, domain + "." + name)
            setattr(module, name, replacements[original])
    cls = swebench._UsageRecovery
    def constructor(self):
        self._by_msg = dict(checked(invoke("experiment.usage.claude.recovery.new", {}))["_by_msg"])
    cls.__init__ = constructor
    def method(original, name):
        signature = inspect.signature(original)
        @functools.wraps(original)
        def native(self, *args, **kwargs):
            bound = signature.bind(self, *args, **kwargs)
            bound.apply_defaults()
            arguments = dict(bound.arguments)
            arguments["self"] = {"_by_msg": list(self._by_msg.items())}
            result = checked(invoke("experiment.usage.claude.recovery." + name, arguments))
            self._by_msg = dict(result["self"]["_by_msg"])
            for message in result["messages"]:
                swebench._log(message)
            return checked(result["value"])
        return native
    for name in ("observe", "synthesize"):
        setattr(cls, name, method(getattr(cls, name), name))
    return replacements
