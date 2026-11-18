"""Mapping, HTTP and identity stand-ins for native provider comparisons."""
import builtins
import json
import os
from pathlib import Path
import sys
import types
import urllib.error

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
from te_harness import swebench, copilot, codex
if "--native" in sys.argv:
    from native_experiment_providers import install
    install()

catalogue_cache,previous_result = None,None
for line in sys.stdin:
    request = json.loads(line)
    calls = []
    integer_limit = sys.get_int_max_str_digits()
    if "int_max_digits" in request: sys.set_int_max_str_digits(request["int_max_digits"])
    saved = {module:(module.os,getattr(module,"_log",None),getattr(module,"pricing",None)) for module in (swebench,copilot,codex)}
    saved_http,saved_cache = codex.urllib,codex._CATALOG
    def encode(value):
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,dict): return {json.dumps(encode(key),sort_keys=True) if isinstance(key,tuple) else str(key):encode(item) for key,item in dict.items(value)}
        if isinstance(value,bytes): return {"bytes":list(value)}
        if isinstance(value,(tuple,list)): return [encode(item) for item in value]
        if isinstance(value,(set,frozenset)): return sorted(value)
        return value
    def record(name,args=None):
        calls.append([name,encode(args)])
        if request.get("fail") == name:
            kind = request.get("error","RuntimeError")
            if kind == "URLError": raise urllib.error.URLError("fixture failure")
            if kind == "ShadowValueError": raise type("ValueError",(Exception,),{})("fixture failure")
            raise getattr(builtins,kind)("fixture failure")
    class Token:
        def __init__(self,name): self.name = name
        def __bool__(self): record("bool:"+self.name);return request.get("truth",True)
        def __format__(self,spec): record("format:"+self.name,spec);return "formatted-"+self.name
        def __str__(self): record("str:"+self.name);return request.get("string","fixture-"+self.name)
    class Environment(dict):
        def get(self,*args): record("get",args);return super().get(*args)
        def __getitem__(self,key): record("item",key);return super().__getitem__(key)
        def pop(self,*args): record("pop",args);return super().pop(*args)
        def __setitem__(self,key,value): record("set",[key,value]);return super().__setitem__(key,value)
    class Cache(dict):
        def __contains__(self,key): record("cache.contains",key);return super().__contains__(key)
        def __getitem__(self,key): record("cache.get",key);return super().__getitem__(key)
        def __setitem__(self,key,value): record("cache.set",[key,value]);return super().__setitem__(key,value)
    environment = Environment(request.get("env",{}))
    for name in request.get("objects",[]): dict.__setitem__(environment,name,Token(name))
    def make_request(*args,**kwargs): record("request",[args,kwargs]);return args
    class Response:
        def __enter__(self): record("enter");return self
        def __exit__(self,kind,error,tb):
            details = {"kind":kind.__name__,"message":str(error),"msg":getattr(error,"msg",None),"pos":getattr(error,"pos",None)} if kind else None
            record("exit",details if request.get("exit_details") else kind.__name__ if kind else None)
            return request.get("suppress",False)
        def read(self):
            record("read")
            if "bytes" in request: return bytes(request["bytes"])
            return request.get("body",'{"data":[{"id":"a"},{"id":"b"}]}').encode()
    def urlopen(*args,**kwargs): record("urlopen",[args,kwargs]);return Response()
    def pricing(value): record("pricing",value)
    def logger(text):
        record("log",text)
        return Token("log_result") if request.get("opaque_log_result") else None
    agent,name = request.get("agent","claude"),request.get("name","resolve_anthropic_key")
    module = {"claude":swebench,"copilot":copilot,"codex":codex}[agent]
    arguments = dict(request.get("arguments",{}))
    if "model_bytes" in request: arguments["model"] = bytes(request["model_bytes"])
    if request.get("explicit") or name == "agent_credential_env": arguments["env"] = environment
    if request.get("opaque_key"): arguments["api_key"] = Token("argument_key")
    if not request.get("keep_cache") or catalogue_cache is None: catalogue_cache = Cache()
    try:
        for item in saved:
            item.os = types.SimpleNamespace(environ=environment)
            if hasattr(item,"pricing"): item.pricing = types.SimpleNamespace(weights_for=pricing)
        swebench._log = logger
        if not request.get("real_http"):
            codex.urllib = types.SimpleNamespace(request=types.SimpleNamespace(Request=make_request,urlopen=urlopen),error=urllib.error)
        codex._CATALOG = catalogue_cache
        value = getattr(module,name)(**arguments)
        if name == "agent_credential_env":
            assert value is environment,"environment identity changed"
            if request.get("opaque_key"): assert dict.__getitem__(environment,"ANTHROPIC_API_KEY") is arguments["api_key"],"key identity changed"
        if name == "resolve_anthropic_key" and dict.get(environment,"ANTHROPIC_API_KEY"):
            assert value is dict.__getitem__(environment,"ANTHROPIC_API_KEY"),"key identity changed"
        if name == "fetch_catalog":
            if value is not None: assert type(value) is frozenset,"catalogue was not frozen"
            if request.get("assert_cached_identity"): assert value is previous_result,"cache copied the catalogue"
            previous_result = value
        response = {"value":encode(value)}
    except BaseException as error: response = {"raised":type(error).__name__,"message":str(error)}
    finally:
        for item,(original_os,original_log,original_pricing) in saved.items():
            item.os = original_os
            if original_log is not None: item._log = original_log
            if original_pricing is not None: item.pricing = original_pricing
        codex.urllib,codex._CATALOG = saved_http,saved_cache
        sys.set_int_max_str_digits(integer_limit)
    response.update(calls=calls,env=encode(environment),cache=encode(catalogue_cache))
    print(json.dumps(response,sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
