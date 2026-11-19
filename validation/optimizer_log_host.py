"""File, clock and stream bindings for native optimizer logging comparisons."""
import builtins
import contextlib
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

ABSENT = object()
for line in sys.stdin:
    request = json.loads(line)
    calls,written = [],[]
    saved = {name:getattr(sw,name,ABSENT) for name in ("os","time","sys","print","open","optimizer_log_path","optimizer_call_kind","_log","_OPT_CTX")}
    mode = request.get("mode","record")
    def encode(value):
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,dict): return {str(key):encode(item) for key,item in dict.items(value)}
        if isinstance(value,(tuple,list)): return [encode(item) for item in value]
        return value
    class DetailError(Exception):
        def __str__(self):
            record("error.str")
            if request.get("mutate_error"): sw._log = lambda message:record("new.error.log",message)
            if request.get("fail_format"): raise RuntimeError("format failed")
            return "\ud800\udc00" if request.get("paired_error") else request.get("error_text","fixture error é😀")
    class FormatError(DetailError):
        def __format__(self,spec): record("error.format",spec);return "formatted error"
    def record(name,args=None):
        calls.append([name,encode(args)])
        if request.get("fail") == name:
            kind = request.get("error","RuntimeError")
            if kind == "DetailError": raise DetailError()
            if kind == "FormatError": raise FormatError()
            if kind == "ShadowException": raise type("Exception",(BaseException,),{})("fixture failure")
            raise getattr(builtins,kind)("fixture failure")
    class Token:
        def __init__(self,name): self.name = name
        def __bool__(self): record("bool:"+self.name);return request.get("truthy",True)
        def __format__(self,spec):
            record("format:"+self.name,spec)
            if request.get("mutate_print"): sw.print = lambda *args,**kwargs:record("new.print",[args,kwargs])
            return request.get("formatted","formatted-"+self.name)
        def __getitem__(self,key):
            record("slice:"+self.name,[key.start,key.stop,key.step])
            return Token("slice") if request.get("opaque_slice") else "slice-"+self.name
        def __len__(self): record("len:"+self.name);return request.get("length",17)
        def __eq__(self,other): record("eq:"+self.name,other);return Token("equality") if request.get("opaque_equality") else request.get("equality",False)
    class Environment(dict):
        def get(self,*args): record("get",args);return super().get(*args)
    class Stream:
        def __init__(self,name): self.name,self.text = name,""
        def write(self,text): record("stream.write",text);self.text += text;return len(text)
        def flush(self): record("flush:"+self.name);return Token("flush_result")
    stream,replacement = Stream("original"),Stream("replacement")
    context = request.get("context",{"agent":"copilot","cell":"arm/repo_s0"})
    if request.get("numeric_keys"): context = {1:"integer","1":"string",None:False}
    arguments = {"msg":"message é😀"} if mode == "verbose" else {"model":"claude-sonnet-5","prompt":"Reply with the single word: ok","text":"response é😀","usage":{"input_tokens":7,"output_tokens":2},"stop_reason":"end_turn"}
    arguments.update(request.get("arguments",{}))
    if request.get("cyclic_usage"): arguments["usage"]["cycle"] = arguments["usage"]
    if request.get("paired_strings"):
        if mode == "verbose": arguments["msg"] += "\ud800\udc00"
        else:
            arguments["prompt"] += "\ud800\udc00"
            arguments["text"] += "\ud800\udc00"
    for key in request.get("objects",[]): arguments[key] = Token(key)
    path = Token("path") if request.get("opaque_path") else request.get("path","/fixture/logs/optimizer.jsonl")
    if request.get("real_file"):
        Path(path).parent.mkdir(parents=True,exist_ok=True)
        Path(path).write_text(request.get("initial_file",""))
    def log_path(): record("path");return path
    def makedirs(path,**kwargs):
        record("makedirs",[path,kwargs])
        if request.get("real_file"): os.makedirs(path,**kwargs)
        return Token("makedirs_result")
    def dirname(path):
        record("dirname",path)
        if request.get("mutate_makedirs"): sw.os.makedirs = lambda *args,**kwargs:record("new.makedirs",[args,kwargs])
        return os.path.dirname(path) if request.get("real_file") else "/fixture/logs"
    def gmtime():
        record("gmtime")
        if request.get("mutate_time"): sw.time.strftime = lambda *args:record("new.strftime",args)
        if request.get("mutate_write"): handle.replaced = lambda line:record("new.write",line)
        return Token("clock")
    def strftime(*args): record("strftime",args);return Token("timestamp") if request.get("opaque_timestamp") else "2026-11-18T12:34:56Z"
    def kind(prompt):
        record("kind",prompt)
        assert prompt is arguments["prompt"],"prompt identity changed"
        if request.get("mutate_context"):
            context["after"] = "not copied"
            context["shared"].append("after")
            arguments["usage"]["changed"] = True
        return request.get("kind","preflight")
    class Handle:
        replaced = None
        @property
        def write(self): record("write.lookup");return self.replaced or self.original_write
        def original_write(self,line): record("write",line);written.append(line);return Token("write_result")
    handle = Handle()
    class FileContext:
        def __enter__(self):
            record("enter")
            if request.get("mutate_exit"): FileContext.__exit__ = lambda *args:record("new.exit")
            return handle
        def __exit__(self,kind,error,tb):
            record("exit",kind.__name__ if kind else None)
            if request.get("exception_state"): record("exit.state",{"active_error":sys.exception() is error,"same_traceback":error is None or error.__traceback__ is tb})
            if request.get("exit_error"): raise getattr(builtins,request["exit_error"])("fixture exit failure")
            return Token("exit_result") if request.get("opaque_exit") else request.get("suppress",False)
    if request.get("static_context"):
        FileContext.__enter__ = staticmethod(lambda: (record("enter"),handle)[1])
        FileContext.__exit__ = staticmethod(lambda *args:record("exit",args[0].__name__ if args[0] else None))
    if request.get("descriptor_context"):
        class Method:
            def __init__(self,name,method): self.name,self.method = name,method
            def __get__(self,instance,owner): record("lookup:"+self.name);return self.method
        FileContext.__enter__ = Method("enter",lambda:(record("enter"),handle)[1])
        FileContext.__exit__ = Method("exit",lambda *args:record("exit",args[0].__name__ if args[0] else None))
    if request.get("missing_special"): delattr(FileContext,request["missing_special"])
    def open_log(*args,**kwargs):
        record("open",[args,kwargs])
        assert args[0] is path,"log path identity changed"
        return builtins.open(*args,**kwargs) if request.get("real_file") else request["file_context"] if "file_context" in request else FileContext()
    def error_log(message):
        record("error.log",message)
        if request.get("paired_error"): record("error.points",list(map(ord,message)))
        return Token("log_result")
    def printing(*args,**kwargs):
        record("print",[args,kwargs])
        if request.get("mutate_stdout"): sw.sys.stdout = replacement
        return Token("print_result")
    try:
        sw.os = types.SimpleNamespace(environ=Environment(request.get("env",{})),path=types.SimpleNamespace(dirname=dirname),makedirs=makedirs)
        sw.time = types.SimpleNamespace(gmtime=gmtime,strftime=strftime)
        sw.sys = sys if request.get("real_print") else types.SimpleNamespace(stdout=stream)
        if not request.get("real_print"): sw.print = printing
        sw.open,sw.optimizer_log_path,sw.optimizer_call_kind,sw._OPT_CTX = open_log,log_path,kind,context
        if mode != "verbose": sw._log = error_log
        owner = web if request.get("webbench") else sw
        with contextlib.redirect_stdout(stream) if request.get("real_print") else contextlib.nullcontext():
            value = getattr(owner,"_log" if mode == "verbose" else "_log_optimizer_call")(**arguments)
        result = {"value":encode(value)}
    except BaseException as error:
        result = {"raised":type(error).__name__,"message":str(error)}
        if request.get("exception_state"): result["context"] = type(error.__context__).__name__ if error.__context__ is not None else None
    finally:
        for name,value in saved.items():
            if value is ABSENT:
                if hasattr(sw,name): delattr(sw,name)
            else: setattr(sw,name,value)
    result.update(calls=calls,written=written,stdout=stream.text)
    if request.get("paired_strings") and mode == "verbose": result["stdout_points"] = list(map(ord,stream.text))
    if request.get("real_file"): result["file"] = Path(path).read_text()
    print(json.dumps(result,sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
