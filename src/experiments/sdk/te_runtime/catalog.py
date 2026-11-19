"""Capture process and filesystem I/O; capture and recorder policy execute in Rust."""
import os
from pathlib import Path
import sys
from types import SimpleNamespace

from .transport import checked

def bind(bridge, module, *, helper_root=None, install_recorder=None):
    invoke = bridge.request
    constants = checked(invoke("experiment.swebench.catalog.constants", {}))
    def callback(method, args):
        if method == "run":
            process = module._run(args[0], **args[1])
            value = dict(returncode=process.returncode, stdout=process.stdout, stderr=process.stderr)
        elif method == "agent":
            process = module.subprocess.run(args[0], input=args[1], capture_output=True, text=True, timeout=600)
            value = dict(returncode=process.returncode, stdout=process.stdout, stderr=process.stderr)
        elif method == "popen":
            module.subprocess.Popen(args, stdout=module.subprocess.DEVNULL, stderr=module.subprocess.DEVNULL)
            value = None
        elif method == "claude_bin": value = module.claude_bin()
        elif method == "image_for": value = module.image_for(args)
        elif method == "live_add": value = module._LIVE_CONTAINERS.add(args)
        elif method == "live_discard": value = module._LIVE_CONTAINERS.discard(args)
        elif method == "log": value = module._log(args)
        elif method == "sleep": value = module.time.sleep(args)
        elif method == "loads": value = module.json.loads(args)
        elif method == "exists": value = module.os.path.exists(args)
        elif method == "read_json":
            with open(args, encoding="utf-8") as stream: value = module.json.load(stream)
        elif method == "write_json":
            module.os.makedirs(module.os.path.dirname(args[0]), exist_ok=True)
            with open(args[0], "w", encoding="utf-8") as stream:
                module.json.dump(args[1], stream, indent=2, sort_keys=True)
            value = None
        elif method == "install_recorder":
            value = install_recorder(args) if install_recorder else drive("install", dict(name=args, helper_root=helper_root))
        elif method == "_exec":
            process = module._run(["docker", "exec", args[0], "bash", "-lc", args[1]], **args[2])
            value = dict(returncode=process.returncode, stdout=process.stdout, stderr=process.stderr)
        elif method == "container_run":
            process = module._run(args[0], timeout=args[1])
            value = dict(returncode=process.returncode, stdout=process.stdout, stderr=process.stderr)
        else: raise ValueError("unsupported catalog callback: " + method)
        return value

    def drive(operation, arguments):
        errors = []
        result = invoke("experiment.swebench.catalog." + operation, arguments)
        while isinstance(result, dict) and "callback" in result:
            method, args = result["method"], result["args"]
            try:
                with bridge.callback_scope():
                    bridge.deliver_signals()
                    value = callback(method, args)
                reply = {"value": value}
            except BaseException as error:
                errors.append(error)
                reply = {"error": dict(kind=type(error).__name__, message=str(error),
                    details={"catalog_error_index":len(errors)-1})}
            result = invoke("experiment.swebench.resume", {"job": result["job"], "reply": reply})
        bridge.deliver_signals()
        if isinstance(result, dict) and "catalog_error_index" in (result.get("details") or {}):
            raise errors[result["details"]["catalog_error_index"]]
        return checked(result)

    def capture(instance_id=module.CAPTURE_INSTANCE, port=None, out_path=None, desc_chars=None, model=None, effort=None):
        arguments = dict(instance_id=instance_id, port=port, out_path=out_path,
            desc_chars=desc_chars, model=model, effort=effort, env=dict(module.os.environ),
            default_out=module.DEFAULT_OUT, recorder_source=module.RECORDER_SRC)
        return drive("capture", arguments)
    return SimpleNamespace(capture=capture,
        system_tools=lambda stream_json_text:checked(invoke("experiment.swebench.catalog.system_tools", dict(stream_json_text=stream_json_text))),
        recorder=constants["recorder"])

def main(bridge, root):
    from .swebench import bind as harness
    module = harness(bridge)
    constants = checked(bridge.request("experiment.swebench.catalog.constants", {}))
    module.CAPTURE_INSTANCE = os.environ.get("TE_CATALOG_INSTANCE", constants["instance"])
    module.DEFAULT_OUT = os.fspath(Path(root) / "src/trace/claude_tool_catalog.json")
    module.RECORDER_SRC = constants["recorder"]
    helpers = Path(root) / "target/container-helpers"
    native = bind(bridge, module, helper_root=os.fspath(helpers) if helpers.is_dir() else None)
    catalog = native.capture()
    output = checked(bridge.request("experiment.swebench.catalog.report", dict(catalog=catalog)))
    sys.stdout.write(output["stdout"])
    sys.stderr.write(output["stderr"])
