"""Object and process bindings for retained SWE-bench scaffold context callbacks."""
import builtins
import inspect
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile

ROOT=Path(__file__).resolve().parents[1]
sys.path[:0]=[str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
from te_harness import swebench as swe
from te_bench.baselines import base,repomap
if "--native" in sys.argv:
    from native_experiment_providers import install
    install()
    previous=swe.ClaudeSwebenchRunner._scaffold_context
    install()
    assert previous is swe.ClaudeSwebenchRunner._scaffold_context

for line in sys.stdin:
    request=json.loads(line);calls,counts=[],{}
    saved={name:getattr(swe,name) for name in ("_run","_log","subprocess","scaffolds_pkg")}
    class Token:
        def __init__(self,name): self.name=name
        def __bool__(self): record("bool:"+self.name);return request.get("booleans",{}).get(self.name,True)
        def __format__(self,spec): record("format:"+self.name,spec);return "opaque "+self.name+str(counts["format:"+self.name])+" é😀"
        def __repr__(self): record("repr:"+self.name);return "opaque "+self.name
        def __ne__(self,other): record("ne:"+self.name,other);return Token("comparison") if request.get("opaque_comparison") else request.get("different",False)
        def __getitem__(self,key): record("slice:"+self.name,[key.start,key.stop,key.step]);return Token("sliced")
    class DetailError(Exception):
        def __str__(self): record("error.str");return "SWE scaffold context failure é😀"
    def encode(value):
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,Context): return {key:encode(item) for key,item in vars(value).items() if key != "_native_merge_helper"}
        if isinstance(value,bytes): return {"bytes":list(value)}
        if isinstance(value,dict): return {key:encode(item) for key,item in dict.items(value)}
        if isinstance(value,(list,tuple)): return [encode(item) for item in value]
        if isinstance(value,subprocess.CompletedProcess): return {"rc":value.returncode,"stdout":encode(value.stdout),"stderr":encode(value.stderr)}
        if callable(value): return {"name":value.__name__,"qualname":value.__qualname__,"module":value.__module__,"signature":str(inspect.signature(value)),"doc":value.__doc__}
        return value
    def record(name,args=None):
        calls.append([name,encode(args),type(sys.exception()).__name__ if sys.exception() is not None else None]);counts[name]=counts.get(name,0)+1
        if request.get("fail") == name and counts[name] == request.get("fail_at",1):
            kind=request.get("error","RuntimeError")
            if kind == "DetailError": raise DetailError()
            raise getattr(builtins,kind)("fixture failure")
    def option(name,default): return Token(name) if name in request.get("objects",[]) else request.get(name,default)
    class Namespace:
        def __init__(self,tag,**values): self.__dict__.update(values);self.tag=tag
        def __getattribute__(self,name):
            if name not in ("tag","__dict__","__class__"): record(object.__getattribute__(self,"tag")+"."+name)
            return object.__getattribute__(self,name)
    class Context:
        def __init__(self,**kwargs): record("context",[list(kwargs),kwargs]);self.__dict__.update(kwargs)
    class Runner(swe.ClaudeSwebenchRunner):
        def __getattribute__(self,name):
            if name == "scaffold_profile": record("profile")
            return super().__getattribute__(name)
    class Overrides(dict):
        def __bool__(self): record("env.bool");return request.get("env_truthy",True)
        def items(self): record("env.items");return request.get("pairs",list(dict.items(self)))
    raw_stdout=option("stdout",b"binary\x00\xff\x80\n")
    raw_stderr=option("stderr",bytes([255])*210+b"\nlast error\x00")
    class Result:
        @property
        def returncode(self): record("returncode");return option("rc",0)
        @property
        def stdout(self): record("stdout");return raw_stdout
        @property
        def stderr(self): record("stderr");return raw_stderr
    def run(args,**kwargs):
        record("run",[args,list(kwargs),kwargs])
        return subprocess.run(args,**kwargs) if request.get("real_process") else Result()
    def host_run(*args,**kwargs):
        record("host_run",[args,list(kwargs),kwargs])
        return subprocess.run(*args,capture_output=True,text=True,**kwargs) if request.get("real_process") else Token("host result")
    def replacement(*args,**kwargs): record("replacement",[args,list(kwargs),kwargs]);return Token("replacement result")
    def logger(*args): record("log",args)
    runner=object.__new__(Runner);object.__setattr__(runner,"scaffold_profile",Token("profile"))
    name=option("name","fixture-container");task=Token("task");directory=Token("run_dir")
    environment=request.get("env",None)
    if request.get("overrides"): environment=Overrides(request.get("env",{"FOO":"first","BAR":42}))
    if request.get("opaque_values"): environment=Overrides({"KEY":Token("value")})
    previous_path=os.environ.get("PATH","");previous_fixture=os.environ.get("TE_DOCKER_FIXTURE_DIRECTORY")
    if request.get("real_process"): os.environ["PATH"]=request["bin_dir"]+os.pathsep+previous_path
    pipeline=request.get("operation") == "pipeline"
    if pipeline:
        owned=Path(request["pipeline_dir"]);owned.mkdir(exist_ok=True)
        os.environ["TE_DOCKER_FIXTURE_DIRECTORY"]=str(owned)
        with tarfile.open(owned/"source.tar","w") as archive:
            for filename in ("keep.py","nested/a b.py","../escape.py","..hidden.py"):
                content=b"class Widget:\n    pass\n";member=tarfile.TarInfo(filename);member.size=len(content);archive.addfile(member,io.BytesIO(content))
        profile=base.AgentProfile(kind="claude",home="/home/task",instructions_file=".github/instructions.md",nav_tools=(),mcp_args=lambda cfg:[],patch_excludes=())
        object.__setattr__(runner,"scaffold_profile",profile)
        task={"problem_statement":"`Widget` fails"};directory=str(owned)
    try:
        swe._run=host_run;swe._log=logger
        swe.subprocess=Namespace("subprocess",run=run,PIPE=subprocess.PIPE if request.get("real_process") else Token("pipe"))
        swe.scaffolds_pkg=Namespace("scaffolds",ScaffoldContext=base.ScaffoldContext if pipeline else Context,ScaffoldError=saved["scaffolds_pkg"].ScaffoldError)
        context=runner._scaffold_context(name,task,directory)
        assert context.container is name and context.task is task and context.run_dir is directory
        if request.get("rebind_run"): swe._run=replacement
        if request.get("second_context"):
            second=runner._scaffold_context("second-container",Token("second task"),Token("second directory"))
            second.exec_("second script")
        operation=request.get("operation","context");script=option("script","fixture script");timeout=option("timeout",23)
        if operation == "pipeline":
            class FixtureRepoMap(repomap.RepoMapScaffold):
                def generate(self,src_dir,idents):
                    record("generate",[src_dir,idents])
                    assert Path(src_dir,"keep.py").is_file() and Path(src_dir,"nested/a b.py").is_file()
                    assert not Path(src_dir,"..hidden.py").exists() and not Path(src_dir).parent.joinpath("escape.py").exists()
                    return "keep.py: Widget\n",""
            value=FixtureRepoMap().setup(context)
            assert value["map_ok"] is True and not owned.joinpath("_repomap_src").exists(),value
            assert "keep.py: Widget" in owned.joinpath("container/testbed/.github/instructions.md").read_text(encoding="utf-8")
            if "--native" in sys.argv:
                import native_plugin
                for name in ("context","exec","copy","read"): assert native_plugin.CALLS["experiment.swebench.provider.swebench.scaffold_"+name] > 0
                for name in ("repomap_setup","repomap_fetch","repomap_idents","repomap_tokens","context_new","context_read","instructions"): assert native_plugin.CALLS["experiment.swebench.provider.scaffold."+name] > 0
        elif operation == "exec": value=context.exec_(script,timeout=timeout,env=environment)
        elif operation == "exec_default": value=context.exec_(script)
        elif operation == "copy": value=context.copy_in(option("host_path","/host/source"),option("container_path","/container/destination"))
        elif operation in ("read","read_default"):
            value=context.read_bytes(script,timeout=timeout) if operation == "read" else context.read_bytes(script)
            if not request.get("real_process"): assert value is raw_stdout
        elif operation == "host": value=context.host_run(["host","args"],timeout=timeout)
        elif operation == "log": value=context.log(Token("message"))
        else: value=context
        output={"value":value}
    except BaseException as error: output={"raised":type(error).__name__,"message":str(error)}
    finally:
        for key,value in saved.items(): setattr(swe,key,value)
        os.environ["PATH"]=previous_path
        if previous_fixture is None: os.environ.pop("TE_DOCKER_FIXTURE_DIRECTORY",None)
        else: os.environ["TE_DOCKER_FIXTURE_DIRECTORY"]=previous_fixture
    output.update(calls=calls)
    print(json.dumps(encode(output),sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
