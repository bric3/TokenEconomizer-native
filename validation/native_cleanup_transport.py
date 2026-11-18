"""A scoped bridge connection for cleanup that can interrupt the shared RPC channel."""
import contextlib
import json
import subprocess

from native_plugin import BRIDGE_POPEN, CALLS, ROOT


@contextlib.contextmanager
def isolated_invoke():
    process = None
    def call(operation,arguments):
        nonlocal process
        if process is None:
            process = BRIDGE_POPEN([str(ROOT/"target/debug/te-bridge")],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True,encoding="utf-8",start_new_session=True)
        process.stdin.write(json.dumps({"op":operation,"args":arguments})+"\n")
        process.stdin.flush()
        line = process.stdout.readline()
        if not line: raise RuntimeError(f"Cleanup bridge exited: {process.poll()}")
        response = json.loads(line)
        CALLS.update([operation])
        if "error" in response: raise ValueError(response["error"])
        return response["result"]
    try: yield call
    finally:
        if process is not None:
            process.stdin.close()
            try: process.wait(timeout=5)
            except subprocess.TimeoutExpired: process.kill();process.wait()
            process.stdout.close()
