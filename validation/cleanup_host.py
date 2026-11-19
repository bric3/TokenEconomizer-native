"""Original/native side-call I/O; the local agent fixture controls workspace permissions."""
import json
import os
from pathlib import Path
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "reference/upstream"))
for argument in sys.argv[1:]:
    if argument.startswith("--base="):
        os.environ["TMPDIR"] = argument[7:]
        tempfile.tempdir = None
    elif argument.startswith("--limit="):
        import resource
        resource.setrlimit(resource.RLIMIT_NOFILE, (int(argument[8:]), resource.getrlimit(resource.RLIMIT_NOFILE)[1]))
from tokeneconomizer.plugin import sidecall, toolcheck
if "--native" in sys.argv:
    from native_plugin import invoke

for line in sys.stdin:
    request = json.loads(line)
    os.environ.update(TE_CLEANUP_CASE=request["case"], TE_CLEANUP_LOG=request["log"])
    try:
        if "--native" in sys.argv:
            operation = "toolcheck.copilot_capture" if request.get("api") == "copilot" else "sidecall.cursor_call"
            result = invoke(operation, dict(prompt="cleanup probe", model="", cli=request["cli"], extra_args=[],
                timeout=30, root=request["root"], home=os.path.expanduser("~"), env=dict(os.environ), process_cwd=os.getcwd(), transport={}))
            while "callback" in result:
                if result["method"] != "pending": raise AssertionError(result)
                result = invoke("sidecall.resume_job", dict(job=result["job"]))
            if "raised" in result: raise RuntimeError(result["message"])
            value = result["value"]
        elif request.get("api") == "copilot":
            value = toolcheck.copilot_capture(cli=request["cli"], timeout=30)
        else:
            value = sidecall.cursor_call("cleanup probe", cli=request["cli"], timeout=30)
        output = {"value":value}
    except Exception as error:
        output = {"raised":type(error).__name__,"message":str(error)}
    print(json.dumps(output,sort_keys=True,ensure_ascii=True,separators=(",",":")),flush=True)
