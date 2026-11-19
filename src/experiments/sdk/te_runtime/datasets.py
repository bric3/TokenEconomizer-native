"""Dataset SDK and iterator I/O; task selection and file generation execute in Rust."""
import os
import json
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
                    if method == "yaml_load":
                        try:
                            import yaml
                        except ImportError:
                            raise SystemExit("PyYAML is required: pip install pyyaml")
                        with open(args, encoding="utf-8") as stream:
                            tasks = yaml.safe_load(stream)
                        value = bool(tasks)
                    elif method == "yaml_iter":
                        current = iter(tasks)
                        value = None
                    elif method == "yaml_next":
                        try:
                            task = next(current)
                        except StopIteration:
                            value = False
                        else:
                            value = True
                            items = {}
                    elif method == "yaml_get":
                        field = args
                        items[field] = task.get(field)
                        value = items[field] is None
                    elif method == "yaml_set":
                        items[field] = args
                        value = None
                    elif method == "yaml_str":
                        items[field] = str(items[field])
                        value = None
                    elif method == "yaml_strip":
                        items[field] = items[field].strip()
                        value = None
                    elif method == "yaml_json":
                        value = json.dumps(items[args], ensure_ascii=False, separators=(",", ":"))
                    elif method == "write_stdout":
                        print(args, end="")
                        value = None
                    elif method == "dataset_splits":
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
        return self._main("swebench_main", program, argv)

    def render(self, yml_path):
        return self._drive("yaml_render", {"path": os.fspath(yml_path)})

    def yaml_main(self, program=None, argv=None):
        return self._main("yaml_main", program, argv)

    def _main(self, operation, program, argv):
        value = self._drive(operation, {"program": program or os.path.basename(sys.argv[0]),
            "env": {key: os.environ[key] for key in (self.constants["projects_env"],) if key in os.environ},
            "argv": sys.argv[1:] if argv is None else argv, "process_cwd": os.getcwd()})
        print(value["stdout"], end="")
        print(value["stderr"], end="", file=sys.stderr)
        if value["exit"] is not None:
            raise SystemExit(value["exit"])
        return value["returncode"]
