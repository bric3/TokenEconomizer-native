"""Owned process and HTTP resources for the original signal-cleanup handler."""
import os
from pathlib import Path
import subprocess
import sys
import threading

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
from te_harness import swebench
if "--native" in sys.argv:
    from native_experiment_providers import install
    install()
from te_harness import webbench as wb
if "--native" in sys.argv: install(wb)
import native_plugin

java,classes,fixture_dir = sys.argv[1:4]
directory = Path(fixture_dir)
events,ready,blocked = (directory/name for name in ("events","ready","blocked"))
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
class Server(wb.ThreadingHTTPServer):
    def shutdown(self): record("shutdown");return super().shutdown()
    def server_close(self): record("server_close");return super().server_close()
class Registry(set):
    def discard(self,value): record("discard "+str(native_plugin.LOCK.locked()));return super().discard(value)
class File:
    def __init__(self): self.actual = open(directory/"evaluator.log","w",encoding="utf-8")
    def close(self): record("close");self.actual.close()
class Session(wb._EvalSession):
    def __setattr__(self,name,value):
        if name == "finished": record("finished "+str(value))
        super().__setattr__(name,value)
server = Server(("127.0.0.1",0),wb._AgentHandler)
thread = threading.Thread(target=server.serve_forever,kwargs={"poll_interval":0.01},daemon=True)
thread.start()
child = subprocess.Popen([java,"-cp",classes,"WebCleanupFixture","node"],stdout=subprocess.PIPE,text=True,encoding="utf-8",start_new_session=True)
node_pid,node_port = map(int,child.stdout.readline().split())
primary = subprocess.Popen([java,"-cp",classes,"WebCleanupFixture","rpc",str(blocked)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True,encoding="utf-8")
session = object.__new__(Session)
for name,value in (("httpd",server),("proc",child),("_log_fh",File()),("finished",False)): object.__setattr__(session,name,value)
wb._LIVE_SESSIONS = Registry([session])
wb._install_signal_cleanup()
native_plugin.PROCESS = primary
ready.write_text(f"{os.getpid()} {os.getpgrp()} {node_pid} {node_port} {server.server_address[1]} {primary.pid}\n",encoding="utf-8")
try:
    native_plugin.invoke("fixture.block",{})
    raise AssertionError("blocked RPC returned")
finally:
    # Normal completion is a test failure; signal termination bypasses this fallback.
    if child.poll() is None: os.killpg(child.pid,9)
    child.wait()
    if primary.poll() is None: primary.kill()
    primary.wait()
    server.shutdown();server.server_close();thread.join(5)
    if not session.finished: session._log_fh.close()
