"""Object, process-group and exception fixtures for evaluator cleanup policy."""
import builtins
import json
from pathlib import Path
import signal
import subprocess
import sys
import types

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
from te_harness import swebench
if "--native" in sys.argv:
    from native_experiment_providers import install
    install()
from te_harness import webbench as wb
if "--native" in sys.argv:
    install(wb)
    previous = wb._EvalSession.close
    install(wb)
    assert wb._EvalSession.close is previous,"cleanup installation is not idempotent"

for line in sys.stdin:
    request = json.loads(line)
    calls,counts = [],{}
    saved = {name:getattr(wb,name) for name in ("os","signal","subprocess","_LIVE_SESSIONS")}
    live,closed = True,False
    def encode(value):
        if value is session: return {"self":True}
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,(list,tuple)): return [encode(item) for item in value]
        if isinstance(value,dict): return {key:encode(item) for key,item in dict.items(value)}
        return value
    class DetailError(Exception):
        def __str__(self): record("error.str");return "fixture failure é😀"
    class SilentTimeout(subprocess.TimeoutExpired):
        def __str__(self): record("timeout.str");return "fixture timeout"
    def record(name,args=None):
        calls.append([name,encode(args)])
        counts[name] = counts.get(name,0)+1
        if request.get("fail") == name and counts[name] == request.get("fail_at",1): failure(request.get("error","RuntimeError"))
    def failure(kind):
        if kind == "TimeoutExpired": raise subprocess.TimeoutExpired("fixture",20)
        if kind == "SilentTimeout": raise SilentTimeout("fixture",20)
        if kind == "DetailError": raise DetailError()
        if kind.startswith("Shadow"): raise type(kind.removeprefix("Shadow"),(Exception,),{})("fixture failure")
        raise getattr(builtins,kind)("fixture failure")
    class Token:
        def __init__(self,name): self.name = name
        def __bool__(self): record("bool:"+self.name);return False
    class Registry:
        def discard(self,value):
            global live
            record("discard",value);live = False
            return Token("discard_result")
    class Server:
        def __getattribute__(self,name):
            if name in ("shutdown","server_close"): record("server."+name)
            return object.__getattribute__(self,name)
        def shutdown(self): record("shutdown");return Token("shutdown_result")
        def server_close(self): record("server_close");return Token("server_close_result")
    outcomes = iter(request.get("waits",["done"]))
    class Process:
        def __getattribute__(self,name):
            if name in ("poll","pid","wait"): record("proc."+name)
            return object.__getattribute__(self,name)
        def poll(self): record("poll");return Token("poll") if request.get("opaque_poll") else request.get("poll")
        @property
        def pid(self):
            record("pid")
            if request.get("mutate_signals"): wb.signal.SIGKILL = 77
            return Token("pid") if request.get("opaque_pid") else 1234
        def wait(self,**kwargs):
            record("wait",[list(kwargs),kwargs])
            outcome = next(outcomes,"done")
            if outcome != "done": failure(outcome)
            return Token("wait_result")
    class File:
        def __getattribute__(self,name):
            if name == "close": record("file.close")
            return object.__getattribute__(self,name)
        def close(self):
            global closed
            record("close");closed = True
            return Token("close_result")
    def getpgid(pid):
        record("getpgid",pid)
        if request.get("mutate_kill"): wb.os.killpg = lambda *args:record("new.killpg",args)
        return Token("group") if request.get("opaque_group") else 2345
    def killpg(*args): record("killpg",args);return Token("kill_result")
    class Signals:
        SIGTERM,SIGKILL = signal.SIGTERM,signal.SIGKILL
        def __getattribute__(self,name):
            if name in ("SIGTERM","SIGKILL"): record("signal."+name)
            return object.__getattribute__(self,name)
    fields = {"httpd":Server(),"proc":Process(),"_log_fh":File(),"finished":False,**request.get("fields",{})}
    for key in request.get("missing",[]): fields.pop(key,None)
    class Session(wb._EvalSession):
        def __getattribute__(self,name):
            if name in fields or name in request.get("missing",[]): record("read:"+name)
            return super().__getattribute__(name)
        def __setattr__(self,name,value): record("write:"+name,value);super().__setattr__(name,value)
    session = object.__new__(Session)
    for key,value in fields.items(): object.__setattr__(session,key,value)
    try:
        wb.os,wb.signal = types.SimpleNamespace(getpgid=getpgid,killpg=killpg),Signals()
        wb.subprocess = types.SimpleNamespace(TimeoutExpired=subprocess.TimeoutExpired)
        wb._LIVE_SESSIONS = Registry()
        value = session.close()
        if request.get("repeat"): session.close()
        result = {"value":encode(value)}
    except BaseException as error: result = {"raised":type(error).__name__,"message":str(error)}
    finally:
        for key,value in saved.items(): setattr(wb,key,value)
    result.update(calls=calls,live=live,closed=closed,state={"finished":vars(session).get("finished")})
    print(json.dumps(result,sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
