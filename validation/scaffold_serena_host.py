"""Object and callback fixtures for the Serena scaffold, without a harness import."""
import builtins
import json
from pathlib import Path
import sys

ROOT=Path(__file__).resolve().parents[1]
if "--sdk" in sys.argv:
    from standalone_scaffolds import runtime
    base,serena=runtime.base,runtime.serena
else:
    sys.path[:0]=[str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
    from te_bench.baselines import base,serena
    if "--native" in sys.argv:
        from native_experiment_providers import install
        install(scaffold_module=base)
        install(scaffold_module=serena)
        previous=serena.SerenaScaffold.setup,serena.SerenaScaffold.mcp_server,serena.SerenaScaffold.nudge
        install(scaffold_module=serena)
        assert previous == (serena.SerenaScaffold.setup,serena.SerenaScaffold.mcp_server,serena.SerenaScaffold.nudge)

assert "te_harness.swebench" not in sys.modules and "te_harness.webbench" not in sys.modules

for line in sys.stdin:
    request=json.loads(line);calls,counts,retained,copied=[],{},{},{}
    saved=serena.INSTALL,serena.SERENA_TOOLS
    class Token:
        def __init__(self,name): self.name=name
        def __bool__(self):
            record("bool:"+self.name)
            value=request.get("booleans",{}).get(self.name,True)
            return value[min(counts["bool:"+self.name]-1,len(value)-1)] if isinstance(value,list) else value
        def __format__(self,spec): record("format:"+self.name,spec);return "opaque "+self.name+str(counts["format:"+self.name])+" é😀"
        def __eq__(self,value): record("eq:"+self.name,value);return Token("comparison") if request.get("opaque_comparison") else request.get("equals",True)
        def __iter__(self): record("iter:"+self.name);return Entries(self.name,request.get("iterables",{}).get(self.name,["Read","Grep"]))
        def strip(self): record("strip:"+self.name);return Token("stripped") if request.get("opaque_stripped") else request.get("stripped","fixture version")
        def __getitem__(self,key): record("slice:"+self.name,[key.start,key.stop,key.step]);return Token("sliced")
        def __radd__(self,other): record("radd:"+self.name,other);return Token("added") if request.get("opaque_added") else "prefix"
        def __add__(self,other): record("add:"+self.name,other);return "wrapped"
    class Entries:
        def __init__(self,name,items): self.name,self.items=name,iter(items)
        def __iter__(self): return self
        def __next__(self): record("next:"+self.name);return next(self.items)
    class DetailError(Exception):
        def __str__(self): record("error.str");return "Serena fixture failure é😀"
    def encode(value):
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,Profile): return "profile"
        if isinstance(value,Context): return "context"
        if isinstance(value,Result): return "install result"
        if isinstance(value,dict): return {key:encode(item) for key,item in dict.items(value)}
        if isinstance(value,(tuple,list)): return [encode(item) for item in value]
        return value
    def record(name,args=None):
        calls.append([name,encode(args),type(sys.exception()).__name__ if sys.exception() is not None else None]);counts[name]=counts.get(name,0)+1
        if request.get("fail") == name and counts[name] == request.get("fail_at",1):
            kind=request.get("error","RuntimeError")
            if kind == "DetailError": raise DetailError()
            raise getattr(builtins,kind)("fixture failure")
    def option(name,default): return Token(name) if name in request.get("objects",[]) else request.get(name,default)
    class Profile:
        def __getattribute__(self,name):
            if name in ("home","kind","nav_tools"): record("profile.get:"+name)
            return object.__getattribute__(self,name)
    class Result:
        @property
        def returncode(self): record("returncode");return option("rc",0)
        @property
        def stdout(self): record("stdout");return option("stdout","  serena fixture é😀\n")
        @property
        def stderr(self): record("stderr");return option("stderr","installation error é😀")
    class Context:
        def __getattribute__(self,name):
            if name in ("profile","exec_","log","write_mcp_config","write_instructions"): record("context.get:"+name)
            return object.__getattribute__(self,name)
        def exec_(self,*args,**kwargs): record("exec",[args,list(kwargs),kwargs]);return result
        def log(self,*args): record("log",args);return Token("ignored")
        def write_mcp_config(self,servers):
            record("write_mcp",servers);retained["servers"]=servers
            return base.ScaffoldContext.write_mcp_config(self,servers) if request.get("real_files") else option("cfg","/home/task/scaffold.mcp.json")
        def write_instructions(self,nudge):
            record("write_instructions",nudge)
            return base.ScaffoldContext.write_instructions(self,nudge) if request.get("real_files") else option("nudged",True)
        def copy_in(self,source,destination):
            record("copy",[source,destination]);content=Path(source).read_text(encoding="utf-8");copied[destination]=content
            target=Path(self.run_dir,"workspace",destination.removeprefix("/testbed/")) if destination.startswith("/testbed/") else Path(destination)
            assert target.is_relative_to(self.run_dir),"copy escaped the fixture directory"
            target.parent.mkdir(parents=True,exist_ok=True);target.write_text(content,encoding="utf-8");return result
    class Scaffold(serena.SerenaScaffold):
        def __getattribute__(self,name):
            if name in ("mcp_server","nudge"):
                record("scaffold.get:"+name)
                if request.get("stub_"+name):
                    def callback(profile): record("call:"+name,profile);return raw_spec if name == "mcp_server" else Token("nudge")
                    return callback
            return super().__getattribute__(name)
    profile=Profile();profile.home=option("home","/home/task");profile.kind=option("kind","claude");profile.nav_tools=option("nav_tools",["Read","Grep","Glob"])
    result=Result();ctx=Context();ctx.profile=profile;raw_spec=Token("spec")
    if request.get("real_files"):
        ctx.run_dir=request["run_dir"];profile.home=str(Path(ctx.run_dir,"home"))
        profile.instructions_file={"claude":"CLAUDE.md","copilot":".github/copilot-instructions.md","codex":"AGENTS.md"}[request["kind"]]
    if "tools" in request: serena.SERENA_TOOLS=request["tools"]
    if request.get("opaque_tools"): serena.SERENA_TOOLS=Token("tools")
    if request.get("opaque_tool"): serena.SERENA_TOOLS=[Token("tool")]
    if "tool_points" in request: serena.SERENA_TOOLS=["".join(map(chr,request["tool_points"]))]
    if request.get("opaque_install"): serena.INSTALL=Token("install")
    try:
        scaffold=Scaffold();operation=request.get("operation","setup")
        value=getattr(scaffold,operation)(ctx if operation == "setup" else profile)
        if operation == "setup" and "mcp_servers" in value:
            assert value["mcp_servers"]["serena"] is retained["servers"]["serena"],"server reference changed"
            if request.get("stub_mcp_server"): assert value["mcp_servers"]["serena"] is raw_spec
        output={"value":value}
    except BaseException as error:
        output={"raised":type(error).__name__,"message":str(error)}
        if request.get("exception_chain"):
            chain,current=[],error.__context__
            while current is not None and len(chain)<5: chain.append(type(current).__name__);current=current.__context__
            output["context"]=chain
    finally: serena.INSTALL,serena.SERENA_TOOLS=saved
    output.update(calls=calls,retained=retained,copied=copied)
    print(json.dumps(encode(output),sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
