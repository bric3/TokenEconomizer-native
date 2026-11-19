"""Archive, process, object and file bindings for repo-map scaffold comparisons."""
import builtins
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile

ROOT=Path(__file__).resolve().parents[1]
if "--sdk" in sys.argv:
    from standalone_scaffolds import runtime
    base,repomap=runtime.base,runtime.repomap
else:
    sys.path[:0]=[str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
    from te_bench.baselines import base,repomap
    if "--native" in sys.argv:
        from native_experiment_providers import install
        install(scaffold_module=base);install(scaffold_module=repomap)
        previous=repomap.extract_idents,repomap.RepoMapScaffold.setup
        install(scaffold_module=repomap)
        assert previous == (repomap.extract_idents,repomap.RepoMapScaffold.setup)

assert "te_harness.swebench" not in sys.modules and "te_harness.webbench" not in sys.modules

for line in sys.stdin:
    request=json.loads(line);calls,counts,extracted,copied=[],{},[],{}
    saved={name:getattr(repomap,name) for name in ("os","io","tarfile","subprocess","shutil","sys","GEN","HEADER","ScaffoldError")}
    original_import=builtins.__import__
    class Token:
        def __init__(self,name): self.name=name
        def __bool__(self):
            record("bool:"+self.name);value=request.get("booleans",{}).get(self.name,True)
            return value[min(counts["bool:"+self.name]-1,len(value)-1)] if isinstance(value,list) else value
        def __len__(self): record("len:"+self.name);return 3
        def __iter__(self): record("iter:"+self.name);return iter(request.get("iterables",{}).get(self.name,["first","second"]))
        def __repr__(self): return "opaque "+self.name
        def __format__(self,spec): record("format:"+self.name,spec);return "opaque "+self.name+str(counts["format:"+self.name])+" é😀"
        def __fspath__(self): record("fspath:"+self.name);return "/run/opaque"
        def __int__(self): record("int:"+self.name);return request.get("integer",2048)
        def __ne__(self,other): record("ne:"+self.name,other);return Token("comparison") if request.get("opaque_comparison") else request.get("different",False)
        def __getitem__(self,key): record("slice:"+self.name,[key.start,key.stop,key.step]);return Token("sliced")
        def __radd__(self,other): record("radd:"+self.name,other);return Token("instructions")
        def strip(self): record("strip:"+self.name);return Token("stripped") if request.get("opaque_stripped") else request.get("stripped","map")
        def startswith(self,prefix): record("startswith:"+self.name,prefix);return request.get("prefix_matches",False)
        def __contains__(self,other): record("contains:"+self.name,other);return request.get("contains",False)
    class DetailError(Exception):
        def __str__(self): record("error.str");return request.get("error_message","repo-map fixture failure é😀")
    class DetailImportError(ImportError):
        def __str__(self): record("error.str");return "missing aider é😀"
    def encode(value):
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,Member): return {"member":object.__getattribute__(value,"_name")}
        if isinstance(value,Context): return "context"
        if isinstance(value,dict): return {key:encode(item) for key,item in dict.items(value)}
        if isinstance(value,(tuple,list)): return [encode(item) for item in value]
        if isinstance(value,bytes): return {"bytes":len(value)}
        if isinstance(value,io.BytesIO): return "buffer"
        if isinstance(value,type): return value.__name__
        return value
    def record(name,args=None):
        calls.append([name,encode(args),type(sys.exception()).__name__ if sys.exception() is not None else None]);counts[name]=counts.get(name,0)+1
        fault=next((item for item in [request,*request.get("failures",[])] if item.get("fail") == name and counts[name] == item.get("fail_at",1)),None)
        if fault:
            kind=fault.get("error","RuntimeError")
            if kind == "DetailError": raise DetailError()
            if kind == "DetailImportError": raise DetailImportError()
            if kind == "ScaffoldError": raise saved["ScaffoldError"]("fixture scaffold failure")
            if kind == "TimeoutExpired": raise subprocess.TimeoutExpired("fixture",3)
            raise getattr(builtins,kind)("fixture failure")
    def option(name,default): return Token(name) if name in request.get("objects",[]) else request.get(name,default)
    class Namespace:
        def __init__(self,tag,**values): self.__dict__.update(values);self.tag=tag
        def __getattribute__(self,name):
            if name not in ("tag","__dict__","__class__"): record(object.__getattribute__(self,"tag")+"."+name)
            return object.__getattribute__(self,name)
    def callback(name,value=None,implementation=None):
        def call(*args,**kwargs):
            observed=args
            if name == "run":
                argv=args[0]
                if "--native" in sys.argv:
                    assert argv[0] == str(ROOT/"target/debug/te-repomap-gen") and argv[1] == "--python",argv
                    argv=[argv[2],repomap.GEN,*argv[3:]]
                observed=([argv[0],"<generator>",*argv[2:]],*args[1:])
                if request.get("real_process"): args=(argv,*args[1:])
            record(name,[observed,list(kwargs),kwargs]);return implementation(*args,**kwargs) if implementation else value
        return call
    class Environment(dict):
        def get(self,*args): record("env.get",args);return dict.get(self,*args)
    class Member:
        def __init__(self,name,regular=True): self._name,self.regular=name,regular
        @property
        def name(self): record("member.name",self._name);return option("member_name",self._name)
        def isfile(self): record("member.isfile",self._name);return option("regular",self.regular)
    class Archive:
        def __enter__(self): record("enter");return self
        def __exit__(self,kind,error,tb): record("exit",[kind.__name__ if kind else None,error is sys.exception()]);return request.get("suppress",False)
        def getmembers(self): record("members");return members
        def extractall(self,*args,**kwargs): record("extract",[args,list(kwargs),kwargs]);extracted.extend(kwargs["members"])
    class Result:
        @property
        def returncode(self): record("returncode");return option("rc",0)
        @property
        def stdout(self): record("stdout");return option("stdout","map fixture é😀\n")
        @property
        def stderr(self): record("stderr");return option("stderr","generator error é😀")
    class Context:
        def __getattribute__(self,name):
            if name in ("task","run_dir","read_bytes","write_instructions","log","profile"): record("context.get:"+name)
            return object.__getattribute__(self,name)
        def read_bytes(self,*args): record("read",args);return blob
        def log(self,*args): record("log",args)
        def write_instructions(self,value):
            record("write",value)
            return base.ScaffoldContext.write_instructions(self,value) if request.get("real_files") else option("written",True)
        def exec_(self,*args,**kwargs): record("exec",[args,list(kwargs),kwargs]);return Result()
        def copy_in(self,source,destination):
            record("copy",[source,destination]);content=Path(source).read_text(encoding="utf-8");copied[destination]=content
            target=Path(self.run_dir,"workspace",destination.removeprefix("/testbed/"))
            assert target.is_relative_to(self.run_dir)
            target.parent.mkdir(parents=True,exist_ok=True);target.write_text(content,encoding="utf-8");return Result()
    class Scaffold(repomap.RepoMapScaffold):
        def __getattribute__(self,name):
            if name in ("fetch_sources","generate","TAR_SCRIPT"):
                record("scaffold.get:"+name)
                if name == "fetch_sources" and request.get("stub_fetch"): return callback("fetch",option("src","/run/source"))
                if name == "generate" and request.get("stub_generate"): return callback("generate",request.get("generated",[option("map","map fixture"),option("err","")]))
            return super().__getattribute__(name)
    def import_module(name,*args,**kwargs):
        if name == "aider.repomap":
            record("import")
            return None
        return original_import(name,*args,**kwargs)
    def make_dirs(path,**kwargs):
        if request.get("real_files"): os.makedirs(path,**kwargs)
    def remove_dir(path,**kwargs):
        if request.get("real_files"): shutil.rmtree(path,**kwargs)
    members=[Member(name,regular) for name,regular in request.get("members",[["keep.py",True],["nested/a b.py",True],["/absolute.py",True],["../parent.py",True],["..hidden.py",True],["nested/../escape.py",True],["link.py",False],["directory",False]])]
    blob=b"" if request.get("empty_blob") else b"archive fixture"
    if request.get("real_files") and not request.get("empty_blob"):
        buffer=io.BytesIO()
        with tarfile.open(fileobj=buffer,mode="w") as archive:
            for member in members:
                item=tarfile.TarInfo(member._name)
                if member.regular:
                    data=b"class Widget:\n    pass\n";item.size=len(data);archive.addfile(item,io.BytesIO(data))
                else: item.type=tarfile.SYMTYPE;item.linkname="../outside";archive.addfile(item)
        blob=buffer.getvalue()
    if request.get("bad_archive"): blob=b"not a tar archive"
    ctx=Context();ctx.run_dir=request.get("run_dir","/run");ctx.task={"problem_statement":request.get("problem","`DataArray.sel` and get_axis_num, CFTimeIndex")};ctx.profile=Namespace("profile",instructions_file="CLAUDE.md")
    environment=Environment(request.get("env",{}))
    if request.get("opaque_budget"): environment["TE_REPOMAP_TOKENS"]=Token("budget")
    try:
        repomap.os=Namespace("os",environ=environment,path=Namespace("path",join=callback("join",implementation=os.path.join),isdir=callback("isdir",implementation=os.path.isdir if request.get("real_files") else lambda path:option("isdir",True))),makedirs=callback("makedirs",implementation=make_dirs))
        repomap.shutil=Namespace("shutil",rmtree=callback("rmtree",implementation=remove_dir))
        repomap.io=Namespace("io",BytesIO=callback("bytesio",implementation=io.BytesIO))
        repomap.tarfile=Namespace("tarfile",open=callback("tar.open",implementation=tarfile.open if request.get("real_files") else lambda **kwargs:Archive()))
        repomap.sys=Namespace("sys",executable=option("executable",request.get("java_bin","/fixture/python")))
        repomap.subprocess=Namespace("subprocess",run=callback("run",implementation=subprocess.run if request.get("real_process") else lambda *a,**k:Result()),TimeoutExpired=subprocess.TimeoutExpired)
        if request.get("real_process"): repomap.GEN=str(ROOT/"validation/RepoMapFixture.java")
        for name in ("HEADER","GEN"):
            if name in request.get("objects",[]): setattr(repomap,name,Token(name))
        builtins.__import__=import_module
        operation=request.get("operation","setup");scaffold=Scaffold()
        if operation == "tokens": value=repomap.map_tokens()
        elif operation == "idents":
            candidate=option("text","`DataArray.sel` get_axis_num CFTimeIndex")
            if "text_points" in request: candidate="".join(map(chr,request["text_points"]))
            if request.get("bytes_text"): candidate=b"raw bytes"
            value=repomap.extract_idents(candidate)
        elif operation == "preflight": value=scaffold.preflight(ctx.profile)
        elif operation == "generate": value=scaffold.generate(option("src_dir","/run/src"),option("idents",["Widget","get_axis_num"]))
        elif operation == "fetch": value=scaffold.fetch_sources(ctx)
        else: value=scaffold.setup(ctx)
        output={"value":value}
    except BaseException as error:
        output={"raised":type(error).__name__,"message":str(error),"suppressed":error.__suppress_context__}
        chain,current=[],error.__context__
        while current is not None and len(chain)<5: chain.append(type(current).__name__);current=current.__context__
        output["context"]=chain
    finally:
        builtins.__import__=original_import
        for name,value in saved.items(): setattr(repomap,name,value)
    output.update(calls=calls,extracted=extracted,copied=copied)
    print(json.dumps(encode(output),sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
