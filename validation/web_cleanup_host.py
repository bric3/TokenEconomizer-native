"""Registry, callable and signal API fixtures for native cleanup coordination."""
import builtins
import json
from pathlib import Path
import sys
import types

ROOT = Path(__file__).resolve().parents[1]
if "--sdk" in sys.argv:
    from standalone_harness import sw as swebench
    webbench=None
else:
    sys.path[:0] = [str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
    from te_harness import swebench, webbench
if "--native" in sys.argv and "--sdk" not in sys.argv:
    from native_experiment_providers import install
    install()
if "--native" in sys.argv and "--sdk" not in sys.argv: install(webbench)

for line in sys.stdin:
    request = json.loads(line)
    swe = request.get("domain") == "swebench"
    wb = swebench if swe else webbench
    registry,cleanup_name = ("_LIVE_CONTAINERS","_cleanup_containers") if swe else ("_LIVE_SESSIONS","_cleanup_sessions")
    saved = {name:getattr(wb,name) for name in ("os","signal","subprocess",registry,cleanup_name)}
    missing = object()
    saved_list = getattr(wb,"list",missing)
    calls,counts,handlers = [],{},[]
    armed = request.get("mode") != "handler"
    class Token:
        def __init__(self,name): self.name = name
        def __bool__(self): record("bool:"+self.name);return False
    def encode(value):
        if isinstance(value,Token): return {"ref":value.name}
        if callable(value):
            if value not in handlers: handlers.append(value)
            return [handlers.index(value),value.__name__,value.__qualname__,value.__module__]
        if isinstance(value,(tuple,list)): return [encode(item) for item in value]
        return value
    class DetailError(Exception):
        def __str__(self): record("error.str");return "fixture cleanup error é😀"
    def record(name,args=None):
        calls.append([name,encode(args)])
        counts[name] = counts.get(name,0)+1
        if armed and request.get("fail") == name and counts[name] == request.get("fail_at",1):
            kind = request.get("error","RuntimeError")
            if kind == "DetailError": raise DetailError()
            if kind.startswith("Shadow"): raise type(kind.removeprefix("Shadow"),(Exception,),{})("fixture failure")
            raise getattr(builtins,kind)("fixture failure")
    class Session:
        def __init__(self,name): self.name = name
        def __getattribute__(self,name):
            if name == "close": record("get.close:"+object.__getattribute__(self,"name"))
            return object.__getattribute__(self,name)
        def close(self):
            record("close:"+self.name)
            if request.get("mutate_registry"): entries.clear()
            return Token("close")
    entries = request.get("entries",[Token("name"+str(index)) if request.get("opaque_names") else "container"+str(index) for index in range(request.get("sessions",3))]) if swe else [Session(str(index)) for index in range(request.get("sessions",3))]
    class Registry:
        def __iter__(self): record("iter");return iter(entries)
        def __len__(self): record("len");return len(entries)
        def __getattribute__(self,name):
            if name == "discard": record("get.discard")
            return object.__getattribute__(self,name)
        def discard(self,value):
            record("discard",value)
            if value in entries: entries.remove(value)
            return Token("discard")
    def run(*args,**kwargs):
        record("run",[args,list(kwargs),kwargs])
        if request.get("mutate_registry"): entries.clear()
        if request.get("mutate_run"): wb.subprocess.run = lambda *args,**kwargs:record("new.run",[args,list(kwargs),kwargs])
        return Token("result")
    class Subprocess:
        def __getattribute__(self,name):
            if name == "run": record("get.run")
            return object.__getattribute__(self,name)
    def copy_registry(value): record("list");return list(value)
    def cleanup(): record("cleanup");return Token("cleanup")
    def getpid():
        record("getpid")
        if request.get("mutate_kill"): wb.os.kill = lambda *args:record("new.kill",args)
        return Token("pid") if request.get("opaque_pid") else 1234
    def kill(*args): record("kill",args);return Token("kill")
    class Signals:
        SIGTERM,SIGINT,SIGHUP,SIG_DFL = 15,2,1,Token("default")
        def __getattribute__(self,name):
            if name in ("SIGTERM","SIGINT","SIGHUP","SIG_DFL","signal"): record("signal."+name)
            if name == "SIG_DFL" and request.get("mutate_reset"): self.signal = lambda *args:record("new.signal",args)
            return object.__getattribute__(self,name)
        def signal(self,*args):
            record("register",args)
            if request.get("mutate_signals"): self.SIGINT,self.SIGHUP = 77,88
            return Token("register")
    try:
        wb.os,wb.signal,wb.subprocess = types.SimpleNamespace(getpid=getpid,kill=kill),Signals(),Subprocess()
        wb.subprocess.run = run
        setattr(wb,registry,Registry())
        if request.get("replace_list"): wb.list = copy_registry
        mode = request.get("mode","cleanup")
        if mode == "cleanup": value = getattr(wb,cleanup_name)(*request.get("args",[]))
        else:
            value = wb._install_signal_cleanup()
            if request.get("repeat"): value = wb._install_signal_cleanup()
            if mode == "handler":
                calls.clear();counts.clear()
                armed = True
                if request.get("replace_cleanup"): setattr(wb,cleanup_name,cleanup)
                value = handlers[-1](Token("signum") if request.get("opaque_signum") else request.get("signum",15),Token("frame"))
        result = {"value":encode(value)}
    except BaseException as error: result = {"raised":type(error).__name__,"message":str(error)}
    finally:
        for key,value in saved.items(): setattr(wb,key,value)
        if saved_list is missing: vars(wb).pop("list",None)
        else: wb.list = saved_list
    result.update(calls=calls,remaining=len(entries))
    print(json.dumps(result,sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
