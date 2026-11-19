"""Environment, path and mutable-context fixtures for optimizer metadata."""
import builtins
import json
import os
from pathlib import Path
import sys
import types

ROOT = Path(__file__).resolve().parents[1]
if "--sdk" in sys.argv:
    from standalone_optimizer import runtime
    sw=web=runtime.claude
else:
    sys.path[:0] = [str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
    from te_harness import swebench as sw, webbench as web
    if "--native" in sys.argv:
        from native_experiment_providers import install
        install()

for line in sys.stdin:
    request = json.loads(line)
    calls,state = [],{}
    saved = sw.os,sw._OPT_CTX,sw._OPT_KINDS
    def encode(value):
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,dict): return {key:encode(item) for key,item in dict.items(value)}
        if isinstance(value,(list,tuple)): return [encode(item) for item in value]
        return value
    def record(name,args=None):
        calls.append([name,encode(args)])
        if request.get("fail") == name: raise getattr(builtins,request.get("error","RuntimeError"))("fixture failure")
    class Token:
        def __init__(self,name): self.name = name
        def __bool__(self): record("bool:"+self.name);return request.get("truthy",True)
        def __fspath__(self): record("fspath:"+self.name);return "/fixture/arm/cell/run"
        def __getitem__(self,key): record("slice",[key.start,key.stop,key.step]);return self
        def __contains__(self,signature): record("contains",signature);return request.get("contains",False)
    class Environment(dict):
        def get(self,*args): record("get",args);return super().get(*args)
    class Context(dict):
        def clear(self):
            record("clear")
            super().clear()
            if request.get("replace_context"): sw._OPT_CTX = replacement
            return Token("clear_result")
        def update(self,*args,**kwargs): record("update",[args,kwargs]);super().update(*args,**kwargs);return Token("update_result")
    context = Context(request.get("context",{"old":"value"}))
    replacement = Context({"replacement":"value"})
    environment = Environment(request.get("env",{}))
    for name in request.get("env_objects",[]): environment[name] = Token(name)
    arguments = dict(request.get("arguments",{}))
    for name in request.get("objects",[]): arguments[name] = Token(name)
    if "signature_index" in request: arguments["prompt"] = request.get("padding","")+saved[2][request["signature_index"]][1]
    if request.get("paired_prefix"): arguments["prompt"] = "\ud800\udc00"+arguments.get("prompt","")
    def path_function(name):
        def call(*args):
            record(name,args)
            if name == "normpath" and request.get("mutate_paths"):
                def new_dirname(*args):
                    record("new.dirname",args)
                    def new_join(*args): record("new.join",args);return os.path.join(*args)
                    def new_basename(*args): record("new.basename",args);return os.path.basename(*args)
                    path.join,path.basename = new_join,new_basename
                    return os.path.dirname(*args)
                path.dirname = new_dirname
            return getattr(os.path,name)(*args)
        return call
    path = types.SimpleNamespace(**{name:path_function(name) for name in ("normpath","dirname","basename","join")})
    method = request.get("method","optimizer_log_path")
    owner = web if request.get("webbench") else sw
    try:
        sw.os = types.SimpleNamespace(environ=environment,path=path)
        sw._OPT_CTX = context
        if "kinds" in request: sw._OPT_KINDS = request["kinds"]
        if request.get("paired_signature"): sw._OPT_KINDS = [("paired","\ud800\udc00")]
        if request.get("opaque_kind"): sw._OPT_KINDS = [(Token("kind"),"match")]
        value = getattr(owner,method)(**arguments)
        if method == "optimizer_log_path" and "TE_OPTIMIZER_LOG" in request.get("env_objects",[]) and request.get("truthy",True): assert value is environment["TE_OPTIMIZER_LOG"],"explicit path identity changed"
        if method == "set_optimizer_context" and "opaque" in arguments: assert sw._OPT_CTX["opaque"] is arguments["opaque"],"context value identity changed"
        if request.get("opaque_kind") and value is not None: assert value is sw._OPT_KINDS[0][0],"kind result identity changed"
        result = {"value":encode(value)}
    except BaseException as error: result = {"raised":type(error).__name__,"message":str(error)}
    finally:
        result.update(context=encode(sw._OPT_CTX),original_context=encode(context),calls=calls)
        sw.os,sw._OPT_CTX,sw._OPT_KINDS = saved
    print(json.dumps(result,sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
