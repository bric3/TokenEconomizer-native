"""Process, filesystem and observer bindings for Copilot optimizer comparisons."""
import builtins
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import types

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
from te_harness import copilot as cp, swebench as sw
if "--native" in sys.argv:
    from native_experiment_providers import install
    install()

for line in sys.stdin:
    request = json.loads(line)
    calls,paths = [],{}
    names = ("copilot_bin","tempfile","subprocess","shutil","run_env","os","read_log_dir","sum_openai_usage","pricing","OPTIMIZER_DENY_TOOLS","OPTIMIZER_EXCLUDE_TOOLS")
    saved = {name:getattr(cp,name) for name in names}
    saved_record = sw._log_optimizer_call
    def encode(value):
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,dict): return {key:encode(item) for key,item in value.items()}
        if isinstance(value,(list,tuple)): return [encode(item) for item in value]
        if isinstance(value,str): return next(("<"+key+">" for key,path in paths.items() if value == path),value)
        return value
    def record(name,args=None):
        calls.append([name,encode(args)])
        if request.get("fail") == name:
            kind = request.get("error","RuntimeError")
            if kind == "TimeoutExpired": raise subprocess.TimeoutExpired("fixture",1)
            if kind == "SubprocessError": raise subprocess.SubprocessError("fixture failure")
            if kind == "ShadowException": raise type("Exception",(BaseException,),{})("fixture failure")
            raise getattr(builtins,kind)("fixture failure")
    class Token:
        def __init__(self,name): self.name,self.strings = name,0
        def __bool__(self): record("bool:"+self.name);return request.get("truthy",True)
        def __str__(self):
            record("str:"+self.name)
            choices = request.get("strings",["fixture-model"])
            value = choices[min(self.strings,len(choices)-1)]
            self.strings += 1
            return value
    arguments = {"prompt":"fixture é😀",**request.get("arguments",{})}
    for name in request.get("objects",[]): arguments[name] = Token(name)
    usage = Token("usage") if request.get("opaque_usage") else request.get("usage",{"input_tokens":7,"output_tokens":2})
    effective = Token("effective") if request.get("opaque_effective") else 1.5
    def binary(): record("binary");return "fixture-copilot"
    def temp(prefix):
        key = "neutral" if "cwd" in prefix else "logdir"
        record("temp:"+key,prefix)
        paths[key] = tempfile.mkdtemp(prefix=prefix) if request.get("real") else "/fixture/"+key
        return paths[key]
    def remove(path,**kwargs):
        record("remove:"+next(key for key,value in paths.items() if value == path),[path,kwargs])
        if request.get("real"): shutil.rmtree(path,**kwargs)
    class Result:
        @property
        def stdout(self):
            record("stdout")
            return Token("stdout") if request.get("opaque_stdout") else request.get("stdout",'{"type":"assistant.message","data":{"content":"answer é😀"}}')
    def run(argv,**kwargs):
        record("run",[argv,kwargs])
        assert argv[2] is arguments["prompt"],"prompt argument identity changed"
        if "binary" in arguments and arguments["binary"]: assert argv[0] is arguments["binary"],"binary identity changed"
        if request.get("real"): return subprocess.run(argv,**kwargs)
        return Result()
    def environment():
        record("run_env")
        if request.get("mutate_run"): cp.subprocess.run = lambda *args,**kwargs:record("replaced.run")
        if request.get("mutate_environ"): cp.os.environ = {"REPLACED":"yes"}
        return request.get("env",{"COPILOT_AUTO_UPDATE":"false","FIXTURE":"yes"})
    def read(path):
        record("read_log",path)
        if request.get("mutate_sum"): cp.sum_openai_usage = lambda *args:record("replaced.sum")
        if request.get("real"): return Path(path,"fixture.log").read_text()
        return "fixture log"
    def sum_usage(text): record("sum",text);return usage
    def observe(model,prompt,text,observed):
        record("record",[model,prompt,text,observed])
        assert model is arguments.get("model"),"observer model identity changed"
        assert prompt is arguments["prompt"],"observer prompt identity changed"
        if usage: assert observed is usage,"observer usage identity changed"
        if request.get("mutate_usage") and isinstance(observed,dict): observed.update(request["mutate_usage"])
        if request.get("mutate_text") and isinstance(text,list): text.append("observer")
        return Token("record_result")
    def pricing(actual): record("pricing",actual);assert actual is usage,"pricing usage identity changed";return effective
    try:
        cp.copilot_bin = binary
        cp.tempfile = types.SimpleNamespace(mkdtemp=temp)
        cp.shutil = types.SimpleNamespace(rmtree=remove)
        cp.subprocess = types.SimpleNamespace(run=run,TimeoutExpired=subprocess.TimeoutExpired,SubprocessError=subprocess.SubprocessError)
        cp.os = types.SimpleNamespace(environ={"BASE":"original","FIXTURE":"base"})
        cp.run_env,cp.read_log_dir,cp.sum_openai_usage = environment,read,sum_usage
        cp.pricing = types.SimpleNamespace(effective_tokens=pricing)
        sw._log_optimizer_call = observe
        if "deny" in request: cp.OPTIMIZER_DENY_TOOLS = request["deny"]
        if "exclude" in request: cp.OPTIMIZER_EXCLUDE_TOOLS = request["exclude"]
        value = cp.optimizer_call(**arguments)
        if isinstance(value,tuple) and value[1] is not None: assert value[1] is effective,"pricing result identity changed"
        result = {"value":encode(value)}
    except BaseException as error: result = {"raised":type(error).__name__,"message":str(error)}
    finally:
        for name,value in saved.items(): setattr(cp,name,value)
        sw._log_optimizer_call = saved_record
        if request.get("real"):
            result["remaining"] = [key for key,path in paths.items() if Path(path).exists()]
            for path in paths.values(): shutil.rmtree(path,ignore_errors=True)
    result["calls"] = calls
    print(json.dumps(result,sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
