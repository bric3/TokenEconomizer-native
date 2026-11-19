"""Object, file and callback bindings for the shared scaffold classes."""
import builtins
import hashlib
import inspect
import io
import json
import os
from pathlib import Path
import shlex
import sys

ROOT=Path(__file__).resolve().parents[1]
if "--sdk" in sys.argv:
    from standalone_scaffolds import base
else:
    sys.path[:0]=[str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
    from te_bench.baselines import base
    if "--native" in sys.argv:
        from native_experiment_providers import install
        install(scaffold_module=base)
        previous=base.AgentProfile.__init__,base.ScaffoldContext.merge_json_file
        install(scaffold_module=base)
        assert previous == (base.AgentProfile.__init__,base.ScaffoldContext.merge_json_file)
assert "te_harness.swebench" not in sys.modules and "te_harness.webbench" not in sys.modules

for line in sys.stdin:
    request=json.loads(line);calls,counts,files,copies,managers=[],{},{},[],[]
    absent=object();saved={name:getattr(base,name,absent) for name in ("os","open","json","hashlib","shlex")}
    class Token:
        def __init__(self,name): self.name=name
        def __bool__(self): record("bool:"+self.name);return request.get("booleans",{}).get(self.name,True)
        def __iter__(self): record("iter:"+self.name);return iter(request.get("iterables",{}).get(self.name,["first","second"]))
        def __len__(self): record("len:"+self.name);return 2
        def __repr__(self): record("repr:"+self.name);return "opaque_path"
        def __format__(self,spec): record("format:"+self.name,spec);return "formatted "+self.name+str(counts["format:"+self.name])
        def __fspath__(self): record("fspath:"+self.name);return request.get("path_value","/run/path")
        def __eq__(self,value): record("eq:"+self.name,value);return Token("comparison") if request.get("opaque_comparison") else request.get("equals",True)
        def __contains__(self,value): record("contains:"+self.name,value);return request.get("contains",False)
        def __getitem__(self,key): record("slice:"+self.name,[key.start,key.stop,key.step]);return Token("stamp")
    class Callback(Token):
        def __call__(self,*args,**kwargs):
            record("call:"+self.name,[args,list(kwargs),kwargs])
            if self.name == "mcp": return Token("mcp_result") if request.get("opaque_mcp") else request.get("mcp_result",["--mcp-config","fixture"])
            if self.name == "copy":
                content=Path(args[0]).read_text(encoding="utf-8") if request.get("real_files") else files[args[0]]
                copies.append([args[1],content]);return result
            if self.name == "exec": return result
            if self.name == "read": return raw_bytes
            return Token("ignored")
    def encode(value):
        if isinstance(value,Callback): return {"callback":value.name}
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,Profile): return "profile"
        if isinstance(value,Context): return "context"
        if isinstance(value,bytes): return {"bytes":list(value)}
        if isinstance(value,dict): return {key:encode(item) for key,item in dict.items(value)}
        if isinstance(value,(tuple,list)): return [encode(item) for item in value]
        if isinstance(value,Manager): return "file"
        if callable(value): return {"name":value.__name__,"qualname":value.__qualname__,"signature":str(inspect.signature(value))}
        return value
    class DetailError(Exception):
        def __str__(self): record("error.str");return "base scaffold failure é😀"
    def record(name,args=None):
        calls.append([name,encode(args),type(sys.exception()).__name__ if sys.exception() is not None else None]);counts[name]=counts.get(name,0)+1
        fault=next((item for item in [request,*request.get("failures",[])] if item.get("fail") == name and counts[name] == item.get("fail_at",1)),None)
        if fault:
            kind=fault.get("error","RuntimeError")
            if kind == "DetailError": raise DetailError()
            raise getattr(builtins,kind)("fixture failure")
    class Namespace:
        def __init__(self,tag,**values): self.__dict__.update(values);self.tag=tag
        def __getattribute__(self,name):
            if name not in ("tag","__dict__","__class__"): record(object.__getattribute__(self,"tag")+"."+name)
            return object.__getattribute__(self,name)
    def callback(name,value=None,implementation=None):
        def call(*args,**kwargs): record(name,[args,list(kwargs),kwargs]);return implementation(*args,**kwargs) if implementation else value
        return call
    class Profile(base.AgentProfile):
        def __getattribute__(self,name):
            if name in ("instructions_file","home","_mcp_args"): record("profile.get:"+name)
            return super().__getattribute__(name)
        def __setattr__(self,name,value): record("profile.set:"+name,value);object.__setattr__(self,name,value)
    class Context(base.ScaffoldContext):
        def __getattribute__(self,name):
            if name in ("profile","run_dir","exec_","copy_in","_read_bytes","_log"): record("context.get:"+name)
            return super().__getattribute__(name)
        def __setattr__(self,name,value): record("context.set:"+name,value);object.__setattr__(self,name,value)
    class Result:
        @property
        def returncode(self): record("returncode");return Token("rc") if request.get("opaque_rc") else request.get("rc",0)
        @property
        def stdout(self): record("stdout");return Token("stdout") if request.get("opaque_stdout") else request.get("stdout","merged")
    class ExitValue:
        def __bool__(self): record("exit.bool");return request.get("suppress",False)
    class Manager:
        def __init__(self,path): self.path,self.actual,self.closed=path,None,False
        def __enter__(self):
            record("enter",self.path)
            self.actual=builtins.open(self.path,"w",encoding="utf-8") if request.get("real_files") else io.StringIO();files[self.path]="";return self
        def write(self,value): record("write",value);count=self.actual.write(value);files[self.path]+=value;return count
        def __exit__(self,kind,error,tb):
            record("exit",[kind.__name__ if kind else None,sys.exception() is error,error is None or error.__traceback__ is tb]);self.closed=True
            if self.actual is not None:self.actual.close()
            return ExitValue() if request.get("opaque_exit") else request.get("suppress",False)
    def open_file(path,*args,**kwargs): record("open",[path,args,list(kwargs),kwargs]);value=Manager(path);managers.append(value);return value
    def unlink(path):
        record("unlink",path)
        if request.get("real_files"): os.unlink(path)
        files.pop(path,None)
    def sha1(value): record("sha1",value);return Namespace("hash",hexdigest=callback("hexdigest",Token("hexdigest") if request.get("opaque_hash") else hashlib.sha1(value).hexdigest()))
    operation=request.get("operation","instructions")
    profile=object.__new__(Profile);context=object.__new__(Context)
    raw_bytes=b"fixture bytes\xff\x00";result=Result();mcp_callback=Callback("mcp")
    profile_args={"kind":"claude","home":request.get("home","/home/task"),"instructions_file":request.get("instructions_file",".github/instructions.md"),"nav_tools":["Read","Grep"],"mcp_args":mcp_callback,"patch_excludes":["CLAUDE.md"],"bin_dir":"/bin","env":request.get("profile_env",None)}
    inputs={"cfg_path":request.get("cfg","/mcp.json"),"script":Token("script"),"timeout":Token("timeout"),"msg":Token("msg"),"text":request.get("text","instructions é😀\n`cmd` $(sub) EOF\n"),"servers":request.get("servers",{"fixture":{"command":"/bin/true"}}),"filename":request.get("filename","scaffold.mcp.json"),"path":request.get("path","/home/task/settings.json"),"patch":request.get("patch",{"hooks":{"PreToolUse":[{"command":"rtk hook claude"}]}})}
    for name in request.get("objects",[]):
        if name in profile_args: profile_args[name]=Token(name)
        elif name in inputs: inputs[name]=Token(name)
    for name,value in {"kind":profile_args["kind"],"home":profile_args["home"],"instructions_file":profile_args["instructions_file"],"_mcp_args":mcp_callback}.items(): object.__setattr__(profile,name,value)
    context_args={"profile":profile,"container":Token("container"),"task":Token("task"),"run_dir":Token("run_dir") if request.get("opaque_run_dir") else request.get("run_dir","/run"),"exec_":Callback("exec"),"copy_in":Callback("copy"),"host_run":Callback("host"),"read_bytes":None if request.get("no_read") else Callback("read"),"log":None if request.get("no_log") else Callback("logger")}
    for name,value in context_args.items(): object.__setattr__(context,{"read_bytes":"_read_bytes","log":"_log"}.get(name,name),value)
    if request.get("constructor_empty",operation in ("profile_new","context_new")): profile.__dict__.clear();context.__dict__.clear()
    try:
        base.os=Namespace("os",path=Namespace("path",join=callback("join",implementation=os.path.join),dirname=callback("dirname",implementation=os.path.dirname)),unlink=unlink,getpid=callback("getpid",77),environ={"PRIVATE":"must not become a profile override"})
        base.json=Namespace("json",dumps=callback("dumps",implementation=json.dumps),dump=callback("dump",implementation=json.dump))
        base.hashlib=Namespace("hashlib",sha1=sha1);base.shlex=Namespace("shlex",quote=callback("quote",implementation=shlex.quote));base.open=open_file
        if operation == "profile_new":
            Profile.__init__(profile,**profile_args)
            assert object.__getattribute__(profile,"home") is profile_args["home"] and object.__getattribute__(profile,"_mcp_args") is profile_args["mcp_args"],"constructor argument identity changed"
            value=None
        elif operation == "profile_mcp": value=profile.mcp_args(inputs["cfg_path"])
        elif operation == "context_new": Context.__init__(context,**context_args);value=None
        elif operation == "read": value=context.read_bytes(inputs["script"],inputs["timeout"]);assert value is raw_bytes
        elif operation == "log": value=context.log(inputs["msg"])
        elif operation == "mcp": value=context.write_mcp_config(inputs["servers"],inputs["filename"])
        elif operation == "merge": value=context.merge_json_file(inputs["path"],inputs["patch"])
        elif operation in ("mounts","preflight","setup"): value=getattr(base.Scaffold(),operation)(profile)
        else: value=context.write_instructions(inputs["text"])
        if operation == "context_new" and request.get("no_log"): value=context._log(inputs["msg"])
        output={"value":value}
    except BaseException as error:
        output={"raised":type(error).__name__,"message":str(error)}
        if request.get("exception_chain"):
            chain,current=[],error.__context__
            while current is not None and len(chain)<5:chain.append(type(current).__name__);current=current.__context__
            output["context"]=chain
    finally:
        for manager in managers:
            if manager.actual is not None:manager.actual.close()
        for name,value in saved.items():
            if value is absent:vars(base).pop(name,None)
            else:setattr(base,name,value)
    output.update(calls=calls,files=files,copies=copies,closed=[manager.closed for manager in managers],profile=vars(profile),state=vars(context))
    # The profile object in context state retains the same identity as the constructed profile.
    if "profile" in output["state"]: output["state"]=dict(output["state"],profile="profile")
    print(json.dumps(encode(output),sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
