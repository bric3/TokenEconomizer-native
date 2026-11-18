"""Factory, signature, environment and mapping fixtures for the native driver."""
import builtins
import contextlib
import importlib
import inspect
import io
import json
import os
from pathlib import Path
import sys
import types

ROOT=Path(__file__).resolve().parents[1]
sys.path[:0]=[str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments"),str(ROOT/"reference/upstream/experiments/scripts")]
import run_experiment as rx
from te_bench import adapters
if "--native" in sys.argv:
    from native_experiment_providers import install
    install(driver_module=rx)

for line in sys.stdin:
    request=json.loads(line)
    calls=[]
    errors=[]
    saved={name:getattr(rx,name) for name in ("os","importlib","inspect","ledger")}
    adapter_classes={name:getattr(adapters,name) for name in ("SwebenchAdapter","WebBenchAdapter","CredDataAdapter","ReposVulAdapter")}
    def record(name,args=None):
        calls.append([name,args])
        if request.get("fail") == name:
            error=getattr(builtins,request.get("error","RuntimeError"))("fixture failure")
            errors.append(error)
            raise error
    class Environment(dict):
        def setdefault(self,key,default=None):
            record("setdefault",[key,default])
            return super().setdefault(key,default)
    environment=Environment(request.get("env",{}))
    def called(**kwargs):
        record("factory",kwargs)
        error=sys.exception()
        return {"kwargs":kwargs,"exception":type(error).__name__ if error is not None else None,"same_error":bool(errors) and error is errors[-1]}
    def zero(): return called()
    def agent(agent=None): return called(agent=agent)
    def model(model=None): return called(model=model)
    def scaffold(scaffold=None): return called(scaffold=scaffold)
    def all_args(agent=None,model=None,scaffold=None): return called(agent=agent,model=model,scaffold=scaffold)
    def kwargs(**kwargs): return called(**kwargs)
    def positional(agent,/,**kwargs): return called(positional_agent=agent,**kwargs)
    factories=types.SimpleNamespace(zero=zero,agent=agent,model=model,scaffold=scaffold,all=all_args,kwargs=kwargs,positional=positional,dictionary=dict)
    def imported(name):
        record("import",name)
        if name == "fixture": return factories
        return importlib.import_module(name)
    def signature(factory):
        record("signature",getattr(factory,"__name__",type(factory).__name__))
        return inspect.signature(factory)
    def adapter_factory(name):
        def construct(*args,**kwargs):
            record("adapter",[name,args,kwargs])
            return {"adapter":name,"args":args,"kwargs":kwargs}
        return construct
    def encode(value):
        if isinstance(value,rx.StubRunner): return {"stub_agent":value.agent,"codex":value._codex}
        if isinstance(value,set): return sorted(value,key=repr)
        if isinstance(value,dict): return {key:encode(item) for key,item in dict.items(value)}
        if isinstance(value,(tuple,list)): return [encode(item) for item in value]
        return value
    class Mapping(dict):
        def __contains__(self,key): record(self.label+".contains",key);return super().__contains__(key)
        def get(self,key,*default): record(self.label+".get",key);return super().get(key,*default)
        def __getitem__(self,key): record(self.label+".item",key);return super().__getitem__(key)
        def __setitem__(self,key,value): record(self.label+".set",[key,value]);return super().__setitem__(key,value)
    row=Mapping(request.get("row",{}));row.label="row"
    srow=Mapping(request.get("srow",{}));srow.label="srow"
    class Input:
        def __init__(self,path): self.path=path
        def __enter__(self): record("enter",self.path);return self
        def read(self): record("read",self.path);return request["documents"][self.path]
        def __exit__(self,kind,error,traceback):
            record("exit",[self.path,kind.__name__ if kind else None,type(sys.exception()).__name__ if sys.exception() else None])
            return request.get("suppress",False)
    def opener(path,encoding=None): record("open",[path,encoding]);return Input(path)
    capture=io.StringIO()
    try:
        rx.os=types.SimpleNamespace(environ=environment)
        rx.importlib=types.SimpleNamespace(import_module=imported)
        rx.inspect=types.SimpleNamespace(signature=signature,Parameter=inspect.Parameter)
        if "keys" in request: rx.ledger=types.SimpleNamespace(OVERHEAD_KEYS=request["keys"])
        if "documents" in request: rx.open=opener
        for name in adapter_classes: setattr(adapters,name,adapter_factory(name))
        with contextlib.redirect_stdout(capture):
            if request.get("operation") == "overhead":
                value=rx.lift_overhead(row,srow)
                assert value is row,"row identity changed"
                for key in srow:
                    if key not in request.get("row",{}) and key in value:
                        assert dict.__getitem__(value,key) is dict.__getitem__(srow,key),"overhead value identity changed"
            elif request.get("operation") == "tasks": value=rx.load_tasks(request["tasks"],request.get("repo",""),request.get("limit",0),request.get("workload"))
            elif request.get("operation") == "workload": value=rx.workload_ids(request["workload"],request.get("repo",""))
            elif request.get("operation") == "group": value=rx._same_group(request.get("task"),request.get("repo",""))
            elif request.get("operation") == "adapter": value=rx.load_adapter(request.get("name"),request.get("tasks"),request.get("repo",""))
            else: value=rx.load_runner(request.get("spec","fixture:all"),request.get("agent","codex"),request.get("model"),request.get("scaffold"))
        output={"value":encode(value)}
    except BaseException as error: output={"raised":type(error).__name__,"message":str(error),"same_error":bool(errors) and error is errors[-1]}
    finally:
        for name,value in adapter_classes.items(): setattr(adapters,name,value)
        if "documents" in request: del rx.open
        for name,value in saved.items(): setattr(rx,name,value)
    output.update(calls=calls,env=dict(environment),stdout=capture.getvalue(),row=encode(row))
    print(json.dumps(output,sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
