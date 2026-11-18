"""Opaque stream, session and filesystem fixtures for Web-Bench coordination."""
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
    previous = wb.WebBenchRunner.begin_stream
    install(wb)
    assert wb.WebBenchRunner.begin_stream is previous,"stream installation is not idempotent"

for line in sys.stdin:
    request = json.loads(line)
    calls,reads = [],{}
    absent = object()
    saved = {name:getattr(wb,name,absent) for name in ("os","_EvalSession","list")}
    def encode(value):
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,Session): return {"session":value.name}
        if isinstance(value,dict): return {key:encode(item) for key,item in dict.items(value)}
        if isinstance(value,(list,tuple)): return [encode(item) for item in value]
        return value
    class DetailError(Exception):
        def __str__(self): calls.append(["error.str",None]);return "fixture failure é😀"
    def record(name,args=None):
        calls.append([name,encode(args)])
        if request.get("fail") == name:
            kind = request.get("error","RuntimeError")
            if kind == "DetailError": raise DetailError()
            raise getattr(builtins,kind)("fixture failure")
    class Token:
        def __init__(self,name): self.name = name
        def __bool__(self):
            record("bool:"+self.name)
            if request.get("mutate_list") and self.name == "tasks": wb.list = lambda value:(record("new.list",value),list(value))[1]
            return request.get("booleans",{}).get(self.name,True)
        def __iter__(self): record("iter:"+self.name);return iter(request.get("items",[{"repo":"owner/project"}]))
        def __len__(self): record("len:"+self.name);return request.get("length",1)
        def __getitem__(self,key): record("item:"+self.name,key);return Token("first") if key == 0 else Token("project")
        def get(self,*args): record("get:"+self.name,args);return request.get("task_project","owner/project")
        def split(self,*args): record("split:"+self.name,args);return [Token("piece" if self.name == "repo" else "project")]
        def __fspath__(self): record("fspath:"+self.name);return request.get("fspath",{}).get(self.name,self.name)
        def __format__(self,spec): record("format:"+self.name,spec);return "formatted-"+self.name
        def __repr__(self): record("repr:"+self.name);return request.get("repr","token-"+self.name)
        def __ne__(self,other): record("ne:"+self.name,other);return Token("comparison") if request.get("opaque_comparison") else request.get("different",False)
        def __add__(self,other): record("add:"+self.name,other);return Token("attempts")
        def __mul__(self,other):
            record("mul:"+self.name,other)
            if request.get("mutate_factory"): wb._EvalSession = lambda *args,**kwargs:(record("new.factory"),Session("replaced"))[1]
            return Token("reply_timeout")
    class Session:
        def __init__(self,name): self.name = name;self.closed = False
        def __getattribute__(self,name):
            if name == "close": record("session.get.close")
            return object.__getattribute__(self,name)
        def close(self):
            record("session.close",self.name)
            self.closed = True
            if request.get("mutate_close"): object.__setattr__(runner,"_session",None)
            return Token("close_result")
        def __bool__(self): record("bool:session");return False
    def list_function(value): record("list",value);return Token("stream") if request.get("opaque_stream") else list(value)
    def join(*args): record("join",args);return os.path.join(*args)
    def isdir(value): record("isdir",value);return os.path.isdir(value) if request.get("real_paths") else Token("exists") if request.get("opaque_exists") else request.get("exists",True)
    def getcwd(): record("getcwd");return request.get("cwd","/fixture/cwd")
    class Paths:
        def __getattribute__(self,name):
            if name in ("join","isdir"): record("path."+name)
            return object.__getattribute__(self,name)
    paths = Paths()
    paths.join,paths.isdir = join,isdir
    original_session = Session("original")
    fields = {"_stream":[{"repo":"old/project"}],"_stream_broken":True,"_session":original_session,"_session_project":"project","repo":"/fixture/checkout","retry":2,"node":Token("node"),"task_timeout":240,"max_workers":4,**request.get("fields",{})}
    for key in request.get("field_objects",[]): fields[key] = Token(key)
    for key in request.get("missing",[]): fields.pop(key,None)
    cls = {"claude":wb.WebBenchRunner,"copilot":wb.CopilotWebBenchRunner,"codex":wb.CodexWebBenchRunner}[request.get("agent","claude")]
    tracked = {*fields,*request.get("missing",[]),"close_session"}
    class Runner(cls):
        def __getattribute__(self,name):
            if name in tracked:
                record("read:"+name)
                reads[name] = reads.get(name,0)+1
                if request.get("mutate_path") and name == "repo":
                    paths.join = lambda *args:(record("new.join",args),os.path.join(*args))[1]
                    paths.isdir = lambda path:(record("new.isdir",path),False)[1]
                if name in request.get("read_values",{}):
                    variants = request["read_values"][name]
                    return variants[min(reads[name]-1,len(variants)-1)]
                if name == "close_session" and request.get("override_close"):
                    return lambda:(record("override.close"),Token("ignored"))[1]
            return super().__getattribute__(name)
        def __setattr__(self,name,value): record("write:"+name,value);super().__setattr__(name,value)
    runner = object.__new__(Runner)
    for key,value in fields.items(): object.__setattr__(runner,key,value)
    arguments = dict(request.get("arguments",{}))
    method = request.get("method","begin_stream")
    if method == "begin_stream": arguments.setdefault("tasks",[{"repo":"owner/project"}])
    if method == "session_for": arguments.setdefault("project","project")
    for key in request.get("objects",[]): arguments[key] = Token(key)
    def factory(*args,**kwargs):
        record("factory",[args,list(kwargs),kwargs])
        assert kwargs["tasks"] is object.__getattribute__(runner,"_stream"),"stream identity changed"
        repo = arguments.get("repo")
        if type(repo) is str and repo and "/" not in repo: assert args[0] is repo,"project argument lost identity"
        tasks = arguments.get("tasks")
        if not repo and type(tasks) is list and tasks and type(tasks[0]) is dict:
            project = tasks[0].get("repo")
            if type(project) is str and project and "/" not in project: assert args[0] is project,"task project lost identity"
        return Session("created")
    try:
        wb.os = types.SimpleNamespace(path=paths,getcwd=getcwd)
        wb._EvalSession,wb.list = factory,list_function
        value = getattr(runner,method)(**arguments)
        if method == "session_for": assert value is object.__getattribute__(runner,"_session"),"session identity changed"
        if request.get("lifecycle"):
            value = runner.session_for(object.__getattribute__(runner,"_session_project"))
            runner.end_stream()
        result = {"value":encode(value)}
    except BaseException as error: result = {"raised":type(error).__name__,"message":str(error)}
    finally:
        for key,value in saved.items():
            if value is absent: delattr(wb,key)
            else: setattr(wb,key,value)
    result.update(calls=calls,state=encode(vars(runner)),original_closed=original_session.closed)
    print(json.dumps(result,sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
