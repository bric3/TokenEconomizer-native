"""Filesystem and object-protocol fixtures for the Web-Bench workspace round trip."""
import builtins
import fnmatch
import json
import os
from pathlib import Path
import shutil
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
    previous = wb.WebBenchRunner._collect
    install(wb)
    assert previous is wb.WebBenchRunner._collect

for line in sys.stdin:
    request = json.loads(line)
    calls,counts,written,pruned,handles = [],{},{},[],[]
    missing = object()
    saved = {key:getattr(wb,key,missing) for key in ("os","shutil","open","set","_load_ignore_globs","_ignored","_ancestors","_MAX_FILE_BYTES","_SKIP_DIRS","_SKIP_FILES")}
    saved_match = fnmatch.fnmatch
    saved_case,saved_normcase = fnmatch.fnmatchcase,os.path.normcase
    real = request.get("real_files",False)
    workspace = request.get("workspace","/ws")
    operation = request.get("operation","collect")
    class Token:
        def __init__(self,name): self.name = name
        def __bool__(self): record("bool:"+self.name);return request.get("booleans",{}).get(self.name,True)
        def __fspath__(self): record("fspath:"+self.name);return workspace
    def encode(value):
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,dict): return {key:encode(item) for key,item in dict.items(value)}
        if isinstance(value,(tuple,list,set)): return [encode(item) for item in value]
        return value
    class DetailError(Exception):
        def __str__(self): record("error.str");return "fixture failure é😀"
    def record(name,args=None):
        calls.append([name,encode(args)])
        counts[name] = counts.get(name,0)+1
        if request.get("fail") == name and counts[name] == request.get("fail_at",1):
            kind = request.get("error","RuntimeError")
            if kind == "DetailError": raise DetailError()
            if kind == "UnicodeDecodeError": raise UnicodeDecodeError("utf-8",b"\xff",0,1,"invalid start byte")
            if kind.startswith("Shadow"): raise type(kind.removeprefix("Shadow"),(Exception,),{})("fixture failure")
            raise getattr(builtins,kind)("fixture failure")
    class OrderedSet(set):
        def __iter__(self): return iter(sorted(set.__iter__(self)))
    class Relative(str):
        def replace(self,*args): return Relative(str.replace(self,*args))
        def __format__(self,spec):
            error = sys.exception()
            record("format",[str(self),type(error).__name__ if error is not None else None])
            return str.__format__(self,spec)
    class Files:
        def __bool__(self): record("files.bool");return request.get("files_truthy",True)
        def items(self): record("files.items");return request.get("pairs",[["a.txt","text"],["nested/a.js",42]])
    class Manager:
        def __init__(self,path,args,kwargs):
            self.path,self.args,self.kwargs,self.actual = path,args,kwargs,None
        def __enter__(self):
            record("enter",self.path)
            if real:
                self.actual = builtins.open(self.path,*self.args,**self.kwargs)
                handles.append(self.actual)
            if "enter_value" in request: return request["enter_value"]
            return self
        def __exit__(self,kind,error,tb):
            info = [self.path,kind.__name__ if kind else None]
            if request.get("exception_state"): info += [sys.exception() is error,error is None or error.__traceback__ is tb]
            record("exit",info)
            if self.actual is not None: self.actual.close()
            return Token("exit_result") if request.get("opaque_exit") else request.get("suppress",False)
        def read(self):
            record("read",self.path)
            if self.actual is not None: return self.actual.read()
            rel = os.path.relpath(self.path,workspace)
            if rel == "binary": raise UnicodeDecodeError("utf-8",b"\xff",0,1,"invalid start byte")
            return request.get("contents",{}).get(rel,"same" if rel == "same" else "updated é😀")
        def __iter__(self):
            record("file.iter",self.path)
            return iter(self.actual if self.actual is not None else request.get("ignore_lines",[]))
        def write(self,value):
            record("write",[self.path,value]);written[self.path] = value
            if self.actual is not None: return self.actual.write(value)
            return Token("written")
    def open_file(path,*args,**kwargs):
        record("open",[path,args,list(kwargs),kwargs])
        return request["context"] if "context" in request else Manager(path,args,kwargs)
    def path_function(name):
        def call(*args,**kwargs):
            record(name,[args,kwargs])
            if name == "isdir" and not real: return request.get("isdir",True)
            if name == "getsize" and not real:
                rel = os.path.relpath(args[0],workspace)
                return request.get("sizes",{}).get(rel,600000 if rel == "big" else 1)
            value = getattr(os.path,name)(*args,**kwargs)
            return Relative(value) if name == "relpath" and request.get("observe_format") else value
        return call
    class Paths:
        def __getattribute__(self,name):
            if name in ("join","dirname","isdir","getsize","relpath"): record("path."+name)
            return object.__getattribute__(self,name)
    paths = Paths()
    for name in ("join","dirname","isdir","getsize","relpath"): setattr(paths,name,path_function(name))
    def makedirs(path,**kwargs):
        record("makedirs",[path,list(kwargs),kwargs])
        if real: os.makedirs(path,**kwargs)
    def remove(path,**kwargs):
        record("rmtree",[path,list(kwargs),kwargs])
        if real: shutil.rmtree(path,**kwargs)
    def walk(path):
        record("walk",path)
        rows = os.walk(path) if real else request.get("walk",[[workspace,["src","node_modules",".hidden","assets"],["same","changed","new","skip.log","package-lock.json",".new","generated","big","binary"]]])
        for root,dirs,names in rows:
            yield root,dirs,names
            pruned.append([root,dirs[:]])
    def ignore_globs(*args):
        record("ignore_globs",args)
        return saved["_load_ignore_globs"](*args) if real else request.get("globs",["*.log"])
    def ignored(*args): record("ignored",args);return saved["_ignored"](*args)
    def ancestors(*args): record("ancestors",args);return saved["_ancestors"](*args)
    def matcher(*args):
        record("match",args)
        return Token("match") if request.get("opaque_match") else request.get("match_result",False)
    def normcase(value):
        record("normcase",value)
        if request.get("mutate_case"): fnmatch.fnmatchcase = matcher
        return saved_normcase(value)
    class Runner(wb.WebBenchRunner):
        def __getattribute__(self,name):
            if name in ("repo","scaffold_answer_excludes"): record("runner."+name)
            return super().__getattribute__(name)
        def scaffold_answer_excludes(self): record("scaffold_excludes");return request.get("scaffold_files",["generated"])
    runner = object.__new__(Runner)
    if not request.get("missing_repo"): runner.repo = request.get("repo","/repo")
    try:
        wb.os = types.SimpleNamespace(path=paths,makedirs=makedirs,walk=walk,sep=os.sep)
        wb.shutil,wb.open,wb.set = types.SimpleNamespace(rmtree=remove),open_file,OrderedSet
        wb._load_ignore_globs,wb._ignored,wb._ancestors = ignore_globs,ignored,ancestors
        if "cap" in request: wb._MAX_FILE_BYTES = request["cap"]
        if "skip_dirs" in request: wb._SKIP_DIRS = request["skip_dirs"]
        if "skip_files" in request: wb._SKIP_FILES = request["skip_files"]
        if request.get("foreign_match"): fnmatch.fnmatch = matcher
        if request.get("foreign_case"): fnmatch.fnmatchcase = matcher
        if request.get("observe_normcase") or request.get("mutate_case"): os.path.normcase = normcase
        ws = Token("workspace") if request.get("opaque_workspace") else workspace
        if operation == "materialize":
            files = Files() if request.get("opaque_files") else request.get("files",{"a.txt":"text","nested/b.js":False,"../escape":"bad","/absolute":"bad","":"bad"})
            value = runner._materialize(ws,files)
        elif operation == "ancestors": value = saved["_ancestors"](request.get("rel","src/nested/a.js"))
        elif operation == "ignored": value = saved["_ignored"](request.get("rel","src/a.log"),request.get("globs",["*.log"]))
        elif operation == "load_ignore_globs": value = saved["_load_ignore_globs"](runner.repo,request.get("project_dir","/repo/projects/project"))
        else:
            value = runner._collect(ws,request.get("request_files",{"same":"same","changed":"old","deleted":"gone",".hidden/old":"old"}),request.get("project_dir","/repo/projects/project"))
        result = {"value":encode(value)}
    except BaseException as error:
        result = {"raised":type(error).__name__,"message":str(error)}
        if request.get("exception_state"): result["context"] = type(error.__context__).__name__ if error.__context__ is not None else None
    finally:
        fnmatch.fnmatch = saved_match
        fnmatch.fnmatchcase,os.path.normcase = saved_case,saved_normcase
        for handle in handles: handle.close()
        for key,value in saved.items():
            if value is missing: vars(wb).pop(key,None)
            else: setattr(wb,key,value)
    result.update(calls=calls,written=written,pruned=pruned)
    print(json.dumps(result,sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
