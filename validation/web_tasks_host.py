"""File, context-manager and JSON-library fixtures for evaluator task loading."""
import builtins
import json
import os
from pathlib import Path
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
    previous = wb._EvalSession._full_task_ids
    install(wb)
    assert wb._EvalSession._full_task_ids is previous,"task loader installation is not idempotent"

for line in sys.stdin:
    request = json.loads(line)
    calls,counts,raised,identifiers = [],{},[],[]
    absent = object()
    saved = {name:getattr(wb,name,absent) for name in ("os","json","open")}
    integer_limit = sys.get_int_max_str_digits()
    handle = None
    def encode(value):
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,dict): return {key:encode(item) for key,item in dict.items(value)}
        if isinstance(value,(tuple,list)): return [encode(item) for item in value]
        if isinstance(value,bytes): return {"bytes":list(value)}
        return value
    class DetailError(Exception):
        def __str__(self): calls.append(["error.str",None]);return "fixture failure é😀"
    def record(name,args=None):
        calls.append([name,encode(args)])
        counts[name] = counts.get(name,0)+1
        if request.get("fail") == name and counts[name] == request.get("fail_at",1):
            kind = request.get("error","RuntimeError")
            if kind == "DetailError": error = DetailError()
            elif kind == "ShadowOSError": error = type("OSError",(Exception,),{})("fixture failure")
            else: error = getattr(builtins,kind)("fixture failure")
            raised.append(error)
            raise error
    class Token:
        def __init__(self,name): self.name = name
        def __bool__(self):
            record("bool:"+self.name)
            if request.get("mutate_decode") and self.name == "stripped": wb.json = types.SimpleNamespace(loads=lambda value:(record("new.loads",value),{"id":"changed"})[1])
            return request.get("booleans",{}).get(self.name,True)
        def __fspath__(self): record("fspath:"+self.name);return request.get("fspath",{}).get(self.name,self.name)
        def strip(self): record("strip:"+self.name);return Token("stripped") if request.get("opaque_stripped") else request.get("stripped",'{"id":"init"}')
    class Decoded:
        def __getattribute__(self,name):
            if name == "get": record("get_function")
            return object.__getattribute__(self,name)
        def get(self,*args):
            record("get",args)
            value = Token("identifier")
            identifiers.append(value)
            return value
    def loads(value):
        record("loads",value)
        try: return Decoded() if request.get("opaque_decoded") else json.loads(value)
        except BaseException as error: raised.append(error);raise
    class Stream:
        def __init__(self): self.values = iter(handle if handle is not None else [Token("line") for _ in request.get("lines",[1])] if request.get("opaque_lines") else request.get("lines",['{"id":"init"}\n','\n','{"id":"task-1"}\n']))
        def __iter__(self): record("iter");return self
        def __next__(self): record("next");return next(self.values)
    class Special:
        def __init__(self,name): self.name = name
        def __get__(self,instance,owner):
            record("bind:"+self.name,owner.__name__)
            name = self.name
            def call(*args):
                if name == "enter":
                    record("enter")
                    if request.get("mutate_exit"): Manager.__exit__ = Special("new.exit")
                    if handle is not None: handle.__enter__()
                    return request["enter_value"] if "enter_value" in request else Stream()
                kind,error,tb = args
                info = {"type":kind.__name__ if kind is not None else None,"same_error":error is None or any(error is item for item in raised),"traceback":tb is not None}
                if request.get("exception_state"): info["active_error"] = sys.exception() is error;info["same_traceback"] = error is None or error.__traceback__ is tb
                record(name,info)
                if handle is not None: handle.__exit__(kind,error,tb)
                return Token("exit_result") if request.get("opaque_exit") else request.get("suppress",False)
            return call
    class Manager:
        __enter__ = Special("enter")
        __exit__ = Special("exit")
    if request.get("missing_special"): delattr(Manager,request["missing_special"])
    def open_file(path,**kwargs):
        global handle
        record("open",[path,list(kwargs),kwargs])
        if request.get("real_files"): handle = builtins.open(path,**kwargs)
        return request["context"] if "context" in request else Manager()
    def join(*args): record("join",args);return os.path.join(*args)
    class Paths:
        def __getattribute__(self,name):
            if name == "join": record("path.join")
            return object.__getattribute__(self,name)
    paths = Paths()
    paths.join = join
    fields = {"repo":"/fixture/repo","project":"project",**request.get("fields",{})}
    for key in request.get("objects",[]): fields[key] = Token(key)
    for key in request.get("missing",[]): fields.pop(key,None)
    class Session(wb._EvalSession):
        def __getattribute__(self,name):
            if name in fields or name in request.get("missing",[]):
                record("read:"+name)
                if request.get("mutate_path") and name == "repo": paths.join = lambda *args:(record("new.join",args),"changed")[1]
            return super().__getattribute__(name)
    session = object.__new__(Session)
    for key,value in fields.items(): setattr(session,key,value)
    try:
        if "int_max_digits" in request: sys.set_int_max_str_digits(request["int_max_digits"])
        wb.os,wb.json,wb.open = types.SimpleNamespace(path=paths),types.SimpleNamespace(loads=loads),open_file
        value = session._full_task_ids()
        if identifiers and isinstance(value,list): assert all(actual is expected for actual,expected in zip(value,identifiers)),"identifier identity changed"
        result = {"value":encode(value)}
    except BaseException as error:
        result = {"raised":type(error).__name__,"message":str(error)}
        if request.get("exception_state"): result["context"] = type(error.__context__).__name__ if error.__context__ is not None else None
    finally:
        if handle is not None: handle.close()
        for key,value in saved.items():
            if value is absent:
                if hasattr(wb,key): delattr(wb,key)
            else: setattr(wb,key,value)
        sys.set_int_max_str_digits(integer_limit)
    result["calls"] = calls
    print(json.dumps(result,sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
