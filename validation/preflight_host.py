"""Opaque request arguments and local optimizer stand-ins for endpoint preflights."""
import builtins
import json
import os
from pathlib import Path
import sys
import types

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
from te_harness import swebench as sw, codex
if "--native" in sys.argv:
    from native_experiment_providers import install
    install()

for line in sys.stdin:
    request = json.loads(line)
    calls = []
    saved = sw.os,codex.os,sw._log,sw.anthropic_optimizer_call,codex.optimizer_call
    def encode(value):
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,dict): return {key:encode(item) for key,item in value.items()}
        if isinstance(value,(tuple,list)): return [encode(item) for item in value]
        return value
    def record(name,args=None):
        calls.append([name,encode(args)])
        if request.get("fail") == name:
            kind = request.get("error","RuntimeError")
            if kind == "ShadowValueError": raise type("ValueError",(Exception,),{})("fixture failure")
            raise getattr(builtins,kind)("fixture failure")
    class Token:
        def __init__(self,name): self.name = name
        def __bool__(self): record("bool:"+self.name);return request.get("truth",True)
        def __format__(self,spec):
            record("format:"+self.name,spec)
            if request.get("format_mutate_log"): sw._log = lambda message:record("formatted.log",message)
            return "formatted-"+self.name
        def strip(self): record("strip:"+self.name);return request.get("stripped","ok")
    class Environment(dict):
        def get(self,*args): record("get",args);return super().get(*args)
    class Routing(dict):
        def __getitem__(self,key): record("routing.item",key);return super().__getitem__(key)
    agent = request.get("agent","claude")
    kwargs = {"configured_model":"model-é😀",**request.get("arguments",{})}
    if agent == "codex": kwargs.setdefault("routing",Routing(base_url="http://fixture/v1",api_key=Token("routing_key")))
    else:
        kwargs.setdefault("label","fixture")
        kwargs.setdefault("remedy","fixture remedy")
        kwargs.setdefault("api_key",Token("key"))
        kwargs.setdefault("bearer",Token("bearer"))
    for name in request.get("objects",[]):
        if name not in ("probe","effective"): kwargs[name] = Token(name)
    reply = request.get("reply",["ok",1.5])
    if isinstance(reply,list):
        reply = list(reply)
        if "probe" in request.get("objects",[]) and len(reply)>0: reply[0] = Token("probe")
        if "effective" in request.get("objects",[]) and len(reply)>1: reply[1] = Token("effective")
    def optimizer(prompt,**parameters):
        record("optimizer",[prompt,list(parameters),parameters])
        for name in ("configured_model","api_key","bearer","routing","base_url"):
            if name in kwargs and name in parameters: assert parameters[name] is kwargs[name],"argument identity changed: "+name
        if request.get("mutate_log"): sw._log = lambda message:record("new.log",message)
        return reply
    def logger(message):
        record("log",message)
        return Token("log_result") if request.get("opaque_log_result") else None
    try:
        environment = Environment(request.get("env",{}))
        sw.os = codex.os = types.SimpleNamespace(environ=environment)
        sw._log = logger
        sw.anthropic_optimizer_call = codex.optimizer_call = optimizer
        value = (codex.preflight_optimizer if agent == "codex" else sw.preflight_optimizer_endpoint)(**kwargs)
        response = {"value":value}
    except BaseException as error: response = {"raised":type(error).__name__,"message":str(error)}
    finally: sw.os,codex.os,sw._log,sw.anthropic_optimizer_call,codex.optimizer_call = saved
    response["calls"] = calls
    print(json.dumps(response,sort_keys=True,separators=(",",":"),ensure_ascii=False),flush=True)
