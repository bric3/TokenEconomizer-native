"""Object, file, generator and subprocess bindings for Web-Bench network guards."""
import builtins
import io
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
from te_harness import swebench
if "--native" in sys.argv:
    from native_experiment_providers import install
    install()
    from native_experiment_net_guard import install as install_guard
    install_guard()
from te_harness import webbench as wb
if "--native" in sys.argv:
    install(wb)
    previous=wb.WebBenchRunner._net_guard_log,wb.WebBenchRunner._install_net_guard,wb.CodexWebBenchRunner._install_net_guard,wb._shq
    install(wb)
    assert previous == (wb.WebBenchRunner._net_guard_log,wb.WebBenchRunner._install_net_guard,wb.CodexWebBenchRunner._install_net_guard,wb._shq)

for line in sys.stdin:
    request=json.loads(line)
    if "--native" in sys.argv:
        import native_plugin
        native_probes_before=native_plugin.CALLS["experiment.net_guard.process"]
    calls,counts,managers,written,removed,mutations=[],{},[],{},[],{}
    absent=object()
    saved={name:getattr(wb,name,absent) for name in ("os","json","sys","net_guard","subprocess","open","_log","any","sum")}
    class Token:
        def __init__(self,name): self.name=name
        def __bool__(self):
            record("bool:"+self.name)
            value=request.get("booleans",{}).get(self.name,True)
            return value[min(counts["bool:"+self.name]-1,len(value)-1)] if isinstance(value,list) else value
        def __format__(self,spec): record("format:"+self.name,spec);return "opaque "+self.name
        def __str__(self): record("str:"+self.name);return "opaque '"+self.name+"' é😀"
        def __fspath__(self): record("fspath:"+self.name);return request.get("path","/run/path")
        def __ne__(self,value): record("ne:"+self.name,value);return Token("comparison") if request.get("opaque_comparison") else request.get("not_equal",True)
        def strip(self): record("strip:"+self.name);return Token("stripped") if request.get("opaque_stripped") else request.get("stripped","stripped line")
    def encode(value):
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,Manager): return {"file":value.path,"mode":value.mode}
        if isinstance(value,dict): return {key:encode(item) for key,item in dict.items(value)}
        if isinstance(value,(tuple,list)): return [encode(item) for item in list.__iter__(value)] if isinstance(value,list) else [encode(item) for item in value]
        return value
    class DetailError(Exception):
        def __str__(self): record("error.str",type(sys.exception()).__name__ if sys.exception() else None);return "guard fixture failure é😀"
    class ObservedOSError(OSError):
        def __str__(self): record("error.str",type(sys.exception()).__name__ if sys.exception() else None);return "guard file failure é😀"
    def record(name,args=None):
        calls.append([name,encode(args)])
        counts[name]=counts.get(name,0)+1
        fault=next((fault for fault in [request,*request.get("failures",[])] if fault.get("fail") == name and counts[name] == fault.get("fail_at",1)),None)
        if fault:
            kind=fault.get("error","RuntimeError")
            if kind == "DetailError": raise DetailError()
            if kind == "ObservedOSError": raise ObservedOSError()
            if kind.startswith("Shadow"): raise type(kind.removeprefix("Shadow"),(Exception,),{})("fixture failure")
            raise getattr(builtins,kind)("fixture failure")
    class Namespace:
        def __init__(self,tag,**values): self.__dict__.update(values);self.tag=tag
        def __getattribute__(self,name):
            if name not in ("tag","__dict__","__class__"): record(object.__getattribute__(self,"tag")+"."+name)
            return object.__getattribute__(self,name)
    def callback(name,value=None,implementation=None):
        def call(*args,**kwargs):
            record(name,[args,list(kwargs),kwargs])
            return implementation(*args,**kwargs) if implementation else value
        return call
    class Mapping(dict):
        def __getattribute__(self,name):
            if name == "setdefault": record(object.__getattribute__(self,"tag")+".setdefault_function")
            return dict.__getattribute__(self,name)
        def setdefault(self,*args): record(self.tag+".setdefault",args);return dict.setdefault(self,*args)
    def mapping(tag,value): result=Mapping(value);result.tag=tag;return result
    class Iterator:
        def __init__(self,tag,items): self.tag,self.iterator=tag,iter(items)
        def __iter__(self): record(self.tag+".iterator.iter");return self
        def __next__(self): record(self.tag+".next");return next(self.iterator)
    class Entries(list):
        def __iter__(self): record("pre.iter");return Iterator("pre",list.__iter__(self))
        def __getattribute__(self,name):
            if name == "append": record("pre.append_function")
            return list.__getattribute__(self,name)
        def append(self,item): record("pre.append",item);return list.append(self,item)
    document=request.get("settings",{"keep":"preserved","hooks":{"other":[{"command":"keep"}]}})
    if isinstance(document,dict):
        document=mapping("settings",document)
        if isinstance(document.get("hooks"),dict):
            hooks=mapping("hooks",document["hooks"])
            for key,value in dict.items(hooks):
                if isinstance(value,list): dict.__setitem__(hooks,key,Entries(value))
            dict.__setitem__(document,"hooks",hooks)
    class ExitValue:
        def __bool__(self): record("exit.bool",type(sys.exception()).__name__ if sys.exception() else None);return request.get("suppress",False)
    class Manager:
        def __init__(self,path,mode): self.path,self.mode,self.closed,self.actual=path,mode,False,None
        def __enter__(self):
            record("enter",[self.path,self.mode])
            if request.get("real_files"):
                self.actual=builtins.open(self.path,self.mode,encoding="utf-8")
            else: self.actual=io.StringIO()
            if self.mode == "w": written[self.path]=""
            return self
        def __exit__(self,kind,error,tb):
            record("exit",[kind.__name__ if kind else None,sys.exception() is error,error is None or error.__traceback__ is tb])
            self.closed=True
            if self.actual is not None: self.actual.close()
            return ExitValue() if request.get("opaque_exit") else request.get("suppress",False)
        def __iter__(self):
            record("file.iter")
            return Iterator("lines",self.actual if request.get("real_files") else [Token("line")] if request.get("opaque_lines") else request.get("lines",["\n"," {} \n","\t","first","é😀"]))
        def read(self): record("read");return self.actual.read() if request.get("real_files") else request.get("base","127.0.0.1 localhost\n\n")
        def write(self,value): record("write",value);result=self.actual.write(value);written[self.path]+=value;return result
    def open_file(path,*args,**kwargs):
        record("open",[path,args,list(kwargs),kwargs]);manager=Manager(path,args[0] if args else "r");managers.append(manager);return manager
    def mkdir(*args,**kwargs):
        record("mkdir",[args,list(kwargs),kwargs])
        if request.get("real_files"): os.makedirs(*args,**kwargs)
    def remove(path):
        record("remove",path);removed.append(path)
        if request.get("real_files"): os.remove(path)
    def load(handle): record("load",handle);return json.load(handle) if request.get("real_files") else document
    def dump(value,handle,**kwargs): record("dump",[value,handle,list(kwargs),kwargs]);return json.dump(value,handle,**kwargs)
    def dumps(value): record("dumps",value);return json.dumps(value)
    class Completed:
        @property
        def returncode(self):
            record("returncode")
            value=request.get("rc",2)
            if request.get("opaque_rc"): return Token("rc")
            return value[min(counts["returncode"]-1,len(value)-1)] if isinstance(value,list) else value
    def run(*args,**kwargs):
        record("run",[args,list(kwargs),kwargs])
        if request.get("real_process"):
            if "--native" in sys.argv:
                from native_plugin import native_cli_process
                return native_cli_process(*args,**kwargs)
            return subprocess.run(*args,**kwargs)
        return Completed()
    def aggregate(name):
        def call(generator):
            record(name,[generator.__name__,generator.__qualname__])
            if request.get("skip_generator"): return Token("aggregate")
            return getattr(builtins,name)(generator)
        return call
    agent=request.get("agent","claude");operation=request.get("operation","install")
    base={"claude":wb.WebBenchRunner,"copilot":wb.CopilotWebBenchRunner,"codex":wb.CodexWebBenchRunner}[agent]
    if agent == "codex" and request.get("middle"):
        class Middle(wb.WebBenchRunner):
            def _install_net_guard(self,*args): record("middle",args);return Token("middle")
        class Combined(base,Middle): pass
        base=Combined
    tracked=("_net_guard_log","agent_home","scaffold_copilot_home","net_guard_settings_path","net_guard_hook_entry","NET_GUARD_HOOK_KEY","NET_GUARD_PROBE","_verify_net_guard","_net_guard_armed")
    class Runner(base):
        def __getattribute__(self,name):
            if name in tracked: record("get:"+name)
            if name == "_net_guard_armed" and request.get("opaque_armed_get"): return Token("armed")
            return super().__getattribute__(name)
        def __setattr__(self,name,value): record("set:"+name,value);mutations[name]=value;object.__setattr__(self,name,value)
        agent_home=staticmethod(callback("agent_home",request.get("agent_home","/run/home")))
        scaffold_copilot_home=staticmethod(callback("copilot_home",request.get("copilot_home","/run/copilot-home")))
    if operation == "install" and not request.get("real_process"):
        Runner._verify_net_guard=staticmethod(callback("verify",Token("verified") if request.get("opaque_verified") else request.get("verified",True)))
    runner=object.__new__(Runner)
    for name,value in request.get("fields",{}).items(): object.__setattr__(runner,name,value)
    inputs={"home":request.get("home","/run/home"),"run_dir_path":request.get("run_dir","/run"),"command":request.get("command","fixture command"),"s":request.get("s","a 'quoted' path é😀")}
    for name in request.get("objects",[]):
        if name in inputs: inputs[name]=Token(name)
        else: object.__setattr__(runner,name,Token(name))
    try:
        paths=Namespace("path",join=callback("join",implementation=os.path.join),dirname=callback("dirname",implementation=os.path.dirname))
        wb.os=Namespace("os",path=paths,makedirs=mkdir,remove=remove,environ=request.get("env",{}))
        wb.json=Namespace("json",load=load,dump=dump,dumps=dumps)
        wb.sys=Namespace("sys",executable=sys.executable if request.get("real_process") else "/fixture/python")
        wb.net_guard=Namespace("net_guard",__file__=saved["net_guard"].__file__ if request.get("real_process") else "/fixture/net_guard.py",LOG_ENV="TE_NET_GUARD_LOG",hosts_block=callback("hosts_block",implementation=saved["net_guard"].hosts_block) if request.get("real_process") else callback("hosts_block","# fixture hosts\n0.0.0.0 github.com"))
        wb.subprocess=Namespace("subprocess",run=run)
        wb.open=open_file;wb._log=callback("log")
        if request.get("aggregate"):
            wb.any=aggregate("any");wb.sum=aggregate("sum")
        if operation == "install": value=runner._install_net_guard(inputs["home"],inputs["run_dir_path"])
        elif operation == "verify": value=runner._verify_net_guard(inputs["home"])
        elif operation == "blocks": value=runner._net_guard_blocks(inputs["run_dir_path"])
        elif operation == "hosts": value=runner._hosts_file(inputs["run_dir_path"])
        elif operation == "settings": value=runner.net_guard_settings_path(inputs["home"],inputs["run_dir_path"])
        elif operation == "entry": value=runner.net_guard_hook_entry(inputs["command"])
        elif operation == "quote": value=wb._shq(inputs["s"])
        elif operation == "class_log": value=base._net_guard_log(inputs["home"])
        else: value=runner._net_guard_log(inputs["home"])
        result={"value":value}
        if "--native" in sys.argv and request.get("real_process") and operation in ("install","verify") and value is True:
            import native_plugin
            assert native_plugin.CALLS["experiment.net_guard.process"] == native_probes_before+1,"guard probe bypassed the native executable"
    except BaseException as error:
        result={"raised":type(error).__name__,"message":str(error)}
        if request.get("exception_chain"):
            chain,current=[],error.__context__
            while current is not None and len(chain)<5: chain.append(type(current).__name__);current=current.__context__
            result["context"]=chain
    finally:
        for manager in managers:
            if manager.actual is not None: manager.actual.close()
        for name,value in saved.items():
            if value is absent: vars(wb).pop(name,None)
            else: setattr(wb,name,value)
    result.update(calls=calls,written=written,removed=removed,closed=[manager.closed for manager in managers],settings=document,mutations=mutations)
    print(json.dumps(encode(result),sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
