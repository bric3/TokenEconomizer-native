"""Offline object and file fixtures for the native benchmark main loop and stub."""
import builtins
import contextlib
import dataclasses
import io
import json
import os
from pathlib import Path
import shutil
import sys
import time
import types

ROOT=Path(__file__).resolve().parents[1]
sys.path[:0]=[str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments"),str(ROOT/"reference/upstream/experiments/scripts")]
import run_experiment as rx
if "--native" in sys.argv:
    from native_experiment_providers import install
    install(driver_module=rx)

for line in sys.stdin:
    request=json.loads(line)
    root=Path(request["root"])
    if root.exists(): shutil.rmtree(root)
    root.mkdir(parents=True)
    calls=[]
    errors=[]
    saved={key:getattr(rx,key) for key in ("os","time","load_env","load_adapter","load_runner","load_tasks","Economizer")}
    argv=sys.argv
    stdout,stderr=io.StringIO(),io.StringIO()
    def data(value):
        if dataclasses.is_dataclass(value): return dataclasses.asdict(value)
        if isinstance(value,(Runner,Adapter)): return type(value).__name__
        if isinstance(value,dict): return {key:data(item) for key,item in value.items()}
        if isinstance(value,(list,tuple)): return [data(item) for item in value]
        return value
    def record(name,args=None):
        calls.append([name,json.loads(json.dumps(data(args)))])
        if request.get("fail") == name:
            error=getattr(builtins,request.get("error","RuntimeError"))("fixture failure")
            errors.append(error)
            raise error
    class Environment(dict):
        def get(self,key,*default): record("env.get",[key,*default]);return super().get(key,*default)
        def __setitem__(self,key,value): record("env.set",[key,value]);return super().__setitem__(key,value)
    environment=Environment(request.get("env",{}))
    def makedirs(path,**kwargs): record("makedirs",[path,kwargs]);return os.makedirs(path,**kwargs)
    class File:
        def __init__(self,path,handle): self.kind="stream" if Path(path).parent.name == "_streams" else "transcript" if Path(path).name == "transcript.jsonl" else "row";self.handle=handle
        def __enter__(self): record(self.kind+".enter");self.handle.__enter__();return self
        def __exit__(self,kind,error,traceback):
            try: record(self.kind+".exit",kind.__name__ if kind else None)
            finally: self.handle.__exit__(kind,error,traceback)
            return request.get("suppress") == self.kind
        def write(self,value): record(self.kind+".write",value);return self.handle.write(value)
    def opener(path,*args,**kwargs): record("open",[path,args,kwargs]);return File(path,open(path,*args,**kwargs))
    class Adapter: pass
    class Runner:
        def run_task(self,task,**kwargs):
            record("run_task",[task,kwargs])
            if request.get("real_stub"): return rx.StubRunner(request.get("agent","claude")).run_task(task,**kwargs)
            path=kwargs["run_dir_path"];os.makedirs(path,exist_ok=True)
            values=request.get("rows",[{"resolved":True,"num_turns":2,"input_tokens":10}])
            row=dict(values[task["_index"]%len(values)])
            return row,path
    def begin(self,tasks,**kwargs): record("begin_stream",[tasks,kwargs])
    def end(self): record("end_stream",type(sys.exception()).__name__ if sys.exception() else None)
    if request.get("begin",True): Runner.begin_stream=begin
    if request.get("end",True): Runner.end_stream=end
    class Economizer:
        def __init__(self,*args,**kwargs): record("economizer",[args,kwargs])
        def injection_prefix(self,task): record("injection_prefix",task);return "injected é😀"
        def build_tools(self,task,i): record("build_tools",[task,i]);return ["WebSearch"]
        def run_kwargs(self,task): record("run_kwargs",task);return request.get("run_kwargs",{})
        def after_task(self,task,row,saved_dir,srow):
            record("after_task",[task,row,saved_dir,srow])
            srow.update(request.get("srow",{}))
            return request.get("opt_tokens",0),"fixture reason"
    def load_env(): record("load_env");return {}
    def adapter(*args): record("load_adapter",args);return Adapter()
    def runner(*args): record("load_runner",[args,dict(environment)]);return Runner()
    def tasks(*args): record("load_tasks",args);return request.get("tasks",[{"instance_id":"first"},{"instance_id":"second"}])
    try:
        rx.os=types.SimpleNamespace(environ=environment,path=os.path,makedirs=makedirs)
        rx.time=types.SimpleNamespace(strftime=time.strftime,gmtime=lambda:time.gmtime(0))
        rx.load_env,rx.load_adapter,rx.load_runner,rx.load_tasks=load_env,adapter,runner,tasks
        rx.Economizer=Economizer
        rx.open=opener
        sys.argv=["te-bench",*request.get("argv",["--arm",request.get("arm","baseline"),"--agent",request.get("agent","claude"),"--tasks-file","fixture-tasks.json","--results-root",str(root),"--repo",request.get("repo","org/pkg-name"),"--seed",str(request.get("seed",0)),*request.get("extra",[])])]
        with contextlib.redirect_stdout(stdout),contextlib.redirect_stderr(stderr):
            if request.get("operation") == "stub":
                stub=rx.StubRunner(request.get("agent","claude"))
                if request.get("method") == "optimizer": value=stub.optimizer_call("fixture")
                else: value=stub.run_task(request.get("task",{}),arm="fixture",injected="",exclude_tools=[],run_dir_path=str(root/"stub"),**request.get("run_kwargs",{}))
                assert isinstance(value,tuple),"stub result lost its tuple type"
                if request.get("method") == "optimizer": assert type(value[1]) is float,"optimizer cost lost its float type"
            else: value=rx.main()
        output={"value":data(value)}
    except BaseException as error: output={"raised":type(error).__name__,"message":str(error),"same_error":bool(errors) and error is errors[-1]}
    finally:
        for key,value in saved.items(): setattr(rx,key,value)
        del rx.open
        sys.argv=argv
    output.update(calls=calls,stdout=stdout.getvalue(),stderr=stderr.getvalue(),env=dict(environment),files={str(path.relative_to(root)):path.read_text(encoding="utf-8") for path in sorted(root.rglob("*")) if path.is_file()})
    print(json.dumps(output,sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
