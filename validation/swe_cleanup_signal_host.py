"""Owned RPC and subprocess fixtures for SWE-bench signal cleanup."""
import os
from pathlib import Path
import subprocess
import sys
import types

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
from te_harness import swebench as owner
if "--native" in sys.argv:
    from native_experiment_providers import install
    install()
import native_plugin

java,classes,fixture_dir = sys.argv[1:4]
directory = Path(fixture_dir)
events,ready,blocked,removed = (directory/name for name in ("events","ready","blocked","removed"))
os.setsid()
def record(value):
    with open(events,"a",encoding="utf-8") as handle: handle.write(value+"\n")
if "--native" in sys.argv:
    import native_cleanup_transport
    bridge_popen = native_cleanup_transport.BRIDGE_POPEN
    def tracked_bridge(*args,**kwargs):
        child = bridge_popen(*args,**kwargs)
        record(f"bridge {child.pid} {os.getpgid(child.pid)}")
        return child
    native_cleanup_transport.BRIDGE_POPEN = tracked_bridge
class Registry(set):
    def discard(self,value):
        assert value == str(removed)
        record("discard "+str(native_plugin.LOCK.locked()))
        super().discard(value)
        record("remaining "+str(len(self)))
def run(command,**kwargs):
    assert command == ["docker","rm","-f",str(removed)] and kwargs == {"capture_output":True}
    record("run "+str(native_plugin.LOCK.locked()))
    # Exercise the real subprocess API without requiring a Docker daemon.
    result = subprocess.run([java,"-cp",classes,"WebCleanupFixture","docker",str(removed)],**kwargs)
    assert result.returncode == 39
    record("completed 39")
    return result
owner.subprocess = types.SimpleNamespace(run=run)
owner._LIVE_CONTAINERS = Registry([str(removed)])
owner._install_signal_cleanup()
primary = subprocess.Popen([java,"-cp",classes,"WebCleanupFixture","rpc",str(blocked)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True,encoding="utf-8")
native_plugin.PROCESS = primary
ready.write_text(f"{os.getpid()} {os.getpgrp()} {primary.pid}\n",encoding="utf-8")
try:
    native_plugin.invoke("fixture.block",{})
    raise AssertionError("blocked RPC returned")
finally:
    if primary.poll() is None: primary.kill()
    primary.wait()
