"""Dataset SDK and iterator I/O; task selection and file generation execute in Rust."""
import os
import sys

from .transport import checked


class Exporter:
    def __init__(self, bridge):
        self.bridge = bridge
        self.constants = checked(bridge.request("experiment.datasets.constants", {}))

    def _drive(self, operation, arguments, streams=None):
        errors = []
        result = self.bridge.request("experiment.datasets." + operation, arguments)
        while isinstance(result, dict) and "callback" in result:
            method, args = result["method"], result["args"]
            try:
                with self.bridge.callback_scope():
                    self.bridge.deliver_signals()
                    if method == "dataset_splits":
                        from datasets import load_dataset
                        split = args["split"]
                        streams = (load_dataset(path, split=split) for path in args["paths"])
                        value = None
                    elif method == "rows_begin":
                        outer = iter(streams)
                        value = None
                    elif method == "next_split":
                        try:
                            rows = next(outer)
                        except StopIteration:
                            value = False
                        else:
                            current = iter(rows)
                            value = True
                    elif method == "next_row":
                        try:
                            row = next(current)
                        except StopIteration:
                            value = {"done": True}
                        else:
                            value = {"done": False, "row": row}
                    else:
                        raise ValueError("unsupported dataset callback: " + method)
                    reply = {"value": value}
            except BaseException as error:
                errors.append(error)
                reply = {"error": {"kind": type(error).__name__, "message": str(error),
                    "details": {"dataset_error_index": len(errors) - 1}}}
            result = self.bridge.request("experiment.datasets.resume", {"job": result["job"], "reply": reply})
        self.bridge.deliver_signals()
        if isinstance(result, dict) and "dataset_error_index" in (result.get("details") or {}):
            raise errors[result["details"]["dataset_error_index"]]
        return checked(result)

    def build(self, rows_by_split, wanted):
        return tuple(self._drive("build", {"wanted": list(wanted)}, rows_by_split))

    def main(self, program=None, argv=None):
        value = self._drive("swebench_main", {"program": program or os.path.basename(sys.argv[0]),
            "argv": sys.argv[1:] if argv is None else argv, "process_cwd": os.getcwd()})
        print(value["stdout"], end="")
        print(value["stderr"], end="", file=sys.stderr)
        if value["exit"] is not None:
            raise SystemExit(value["exit"])
        return value["returncode"]
