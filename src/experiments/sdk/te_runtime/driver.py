"""Object, standard-library and external-runner bindings; experiment policy runs in Rust."""
import argparse
import builtins
import dataclasses
import importlib
import inspect
import json
import operator
import os
import time
from types import SimpleNamespace


def checked(value):
    if not isinstance(value,dict) or "raised" not in value: return value
    details=value.get("details") or {}
    if "errno" in details: raise OSError(details["errno"],details["strerror"],details["filename"])
    if value["raised"] == "JSONDecodeError": raise json.JSONDecodeError(value["message"],details["doc"],details["pos"])
    if value["raised"] == "UnicodeDecodeError":
        raise UnicodeDecodeError("utf-8",bytes(details["bytes"]),details["start"],details["end"],details["reason"])
    raise getattr(builtins,value["raised"])(value["message"])


def special(instance,name):
    missing=object()
    method=inspect.getattr_static(type(instance),name,missing)
    if method is missing:
        kind=type.__getattribute__(type(instance),"__name__")
        raise TypeError(f"'{kind}' object does not support the context manager protocol (missed {name} method)")
    descriptor=getattr(type(method),"__get__",None)
    return descriptor(method,instance,type(instance)) if descriptor is not None else method


def in_exception(error,function,*args,**kwargs):
    if error is None: return function(*args,**kwargs)
    traceback=BaseException.__traceback__.__get__(error)
    try: raise error
    except BaseException:
        BaseException.__traceback__.__set__(error,traceback)
        return function(*args,**kwargs)


class Facade:
    def __init__(self, host, config, *, agent,repo,results_root,fresh,optimizer_model="agent",max_turns=80,runner=None,adapter=None):
        self.host,self.adapter,self.runner=host,adapter,runner
        self.runners={}
        self.state=None
        self._call("new",dict(config=config,options=dict(agent=agent,repo=repo,results_root=results_root,fresh=fresh,optimizer_model=optimizer_model,max_turns=max_turns),runner=runner is not None,adapter=True))

    def _call(self, name, arguments):
        replies=[]
        while True:
            result=self.host.bridge.request("experiment.facade."+name,dict(arguments,replies=replies,self=self.state,
                native_adapter=self.adapter,runners={key:True for key in self.runners},propose_every=self.host.propose_every,home=os.path.expanduser("~")))
            self.host.propose_every=result["propose_every"]
            if "callback" not in result:
                self.host.bridge.deliver_signals()
                for message in result["messages"]: print(message)
                if "self" in result: self.state=result["self"]
                if "srow" in arguments:
                    arguments["srow"].clear();arguments["srow"].update(result["srow"])
                return checked(result["value"])
            try:
                self.host.bridge.deliver_signals()
                if result["callback"] == "resolve":
                    method=self.host.optimizer(self.runner)
                    if method is not None: self.runners[result["slot"]]=method
                    value=method is not None
                elif result["callback"] == "factory_available": value=False
                elif result["callback"] == "runner": value=self.runners[result["slot"]](result["prompt"],result["model"])
                elif result["callback"] == "estimate":
                    value=self.runners[result["slot"]].estimate_effective(result["prompt"],result["raw"])
                else: raise ValueError("unsupported facade callback: "+result["callback"])
                replies.append({"value":value})
            except Exception as error: replies.append({"error":type(error).__name__+": "+str(error)})

    def injection_prefix(self,task=None): return self._call("injection_prefix",{"task":task})
    def build_tools(self,task,i): return self._call("build_tools",{"task":task,"i":i})
    def _stage(self,name,digest):
        replies=[]
        while True:
            result=checked(self.host.bridge.request("spiral_stage."+name,dict(self=self.state["spiral"],runner="spiral" in self.runners,adapter=False,digest=digest,replies=replies)))
            if "callback" not in result:
                self.host.bridge.deliver_signals()
                self.state["spiral"]=result["self"]
                return checked(result["value"])
            try:
                self.host.bridge.deliver_signals()
                if result["callback"] == "runner": value=self.runners["spiral"](result["prompt"],result["model"])
                elif result["callback"] == "estimate": value=self.runners["spiral"].estimate_effective(result["prompt"],result["raw"])
                else: raise ValueError("unsupported spiral callback: "+result["callback"])
                replies.append({"value":value})
            except Exception as error: replies.append({"error":str(error)})

    def run_kwargs(self,task):
        value=self._call("run_kwargs",{"task":task})
        if "spiral_judge" in value:
            value["spiral_judge"]=(lambda digest:self._stage("judge",digest)) if self.state["spiral"]["spiral_judge"] else None
            value["spiral_nudge_writer"]=(lambda digest:self._stage("nudge_writer",digest)) if self.state["spiral"]["spiral_nudge_writer"] else None
        if value.get("codex_spiral_policy") is not None: value["codex_spiral_policy"]=self.host.policy(**value["codex_spiral_policy"])
        return value
    def after_task(self,task,row,run_dir_path,srow): return tuple(self._call("after_task",dict(task=task,row=row,run_dir_path=run_dir_path,srow=srow)))


class Driver:
    def __init__(self,bridge,root):
        self.bridge=bridge
        constants=bridge.request("experiment.driver.constants",{})
        self.propose_every=constants["propose_every"]
        adapters=bridge.request("experiment.adapters.constants",{})
        self.adapter_env=[adapters[key] for key in ("SWEBENCH_TASKS_ENV","PROJECTS_ROOT_ENV","CREDDATA_ROOT_ENV","REPOSVUL_ROOT_ENV")]
        registry=bridge.request("experiment.arms.registry",{})
        policy=bridge.request("codex_spiral.constants",{})["DEFAULT_POLICY"]
        self.policy=dataclasses.make_dataclass("CodexSpiralPolicy",[(key,type(value),dataclasses.field(default=value)) for key,value in policy.items()],namespace={"__module__":__name__})
        def threshold(policy,turns): return self.policy(**checked(bridge.request("codex_spiral.policy.at_threshold",{"self":dataclasses.asdict(policy),"turns":turns})))
        self.policy.at_threshold=threshold
        def factory(name): return lambda: bridge.request("experiment.arms.config",{"name":name})
        self.module=SimpleNamespace(os=os,json=json,time=time,argparse=argparse,inspect=inspect,importlib=importlib,
            _REPO_ROOT=os.fspath(root),__doc__=constants["description"],ledger=SimpleNamespace(OVERHEAD_KEYS=tuple(constants["overhead_keys"])),
            ARMS={key:SimpleNamespace(mode=value["mode"],cfg=factory(key),env=value["env"],scaffold=value["scaffold"]) for key,value in registry["arms"].items()},SCAFFOLD_ARMS=registry["scaffolds"])
        host=self
        class StubRunner:
            def __init__(self,agent): host.call("stub_new",dict(self=self,agent=agent))
            def optimizer_call(self,prompt,model="agent"): return host.call("stub_optimizer",dict(self=self,prompt=prompt,model=model))
            def _write_transcript(self,run_dir_path): return host.call("stub_transcript",dict(self=self,run_dir_path=run_dir_path))
            def run_task(self,task,*,arm,injected,exclude_tools,run_dir_path,**run_kwargs):
                return host.call("stub_task",dict(self=self,task=task,arm=arm,injected=injected,exclude_tools=exclude_tools,run_dir_path=run_dir_path,run_kwargs=run_kwargs))
        self.module.StubRunner=StubRunner
        self.module.Economizer=lambda *args,**kwargs:Facade(self,*args,**kwargs)
        self.module.needs_optimizer=lambda config:bridge.request("config.needs_optimizer",{"config":config})
        for name in ("main","load_runner","load_adapter","workload_ids","_same_group","load_tasks","lift_overhead","env_path","_parse","load_env","require_keys"):
            setattr(self.module,name,getattr(self,name))

    def main(self): return self.call("main",{})
    def load_runner(self,spec,agent,model=None,scaffold=None): return self.call("load_runner",dict(spec=spec,agent=agent,model=model,scaffold=scaffold))
    def load_adapter(self,name,tasks_file,repo=""): return self.call("load_adapter",dict(name=name,tasks_file=tasks_file,repo=repo))
    def workload_ids(self,workload_file,repo): return self.call("workload_ids",dict(workload_file=workload_file,repo=repo))
    def _same_group(self,task,repo): return self.call("_same_group",dict(task=task,repo=repo))
    def load_tasks(self,tasks_file,repo,limit,workload=None): return self.call("load_tasks",dict(tasks_file=tasks_file,repo=repo,limit=limit,workload=workload))
    def lift_overhead(self,row,srow): return self.call("lift_overhead",dict(row=row,srow=srow))
    def env_path(self,path=None): return self.call("env_path",{"path":path})
    def _parse(self,text): return self.call("_parse",{"text":text})
    def load_env(self,path=None,override=False): return self.call("load_env",dict(path=path,override=override))
    def require_keys(self,*names): return self.call("require_keys",{"names":names})

    def optimizer(self,runner):
        parameters=dict(present=runner is not None,has_optimizer=hasattr(runner,"optimizer_call"),callable=callable(runner))
        result=self.bridge.request("experiment.runner.optimizer_method",parameters)
        if isinstance(result,dict) and result.get("callback") == "representation":
            result=self.bridge.request("experiment.runner.optimizer_method",dict(parameters,representation=repr(runner)))
        method=checked(result)
        if method is None: return None
        if method == "__call__": return runner
        return lambda prompt,model="agent":getattr(runner,method)(prompt,model)

    def _adapters(self):
        def create(name,arguments): return self.bridge.request("experiment.adapters.new",dict(name=name,arguments=arguments,env={key:os.environ[key] for key in self.adapter_env if key in os.environ}))
        return dict(SwebenchAdapter=lambda tasks_path=None:create("swebench",{"tasks_path":tasks_path}),
            WebBenchAdapter=lambda project:create("webbench",{"project":project}),CredDataAdapter=lambda:create("creddata",{}),ReposVulAdapter=lambda:create("reposvul",{}))

    def call(self,name,values):
        with self.bridge.frames(): return self._call(name,values)

    def _call(self,name,values):
        values=dict(values)
        errors=[]
        def error_object(info): return errors[info["details"]["provider_error_index"]]
        def invoke(parameters,handled=False,extra=None):
            arguments=[values[key] for key in parameters[0]]
            keywords={key:values[slot] for key,slot in parameters[1]}
            if extra is not None:
                values["called"]=values["function"](*arguments,**keywords,**values[extra])
            elif handled: values["called"]=in_exception(values.get("caught_error"),values["function"],*arguments,**keywords)
            else: values["called"]=values["function"](*arguments,**keywords)
        result=self.bridge.request("experiment.swebench.provider.driver."+name,{})
        while isinstance(result,dict) and "callback" in result:
            method,p=result["method"],result["args"]
            try:
                with self.bridge.callback_scope():
                    self.bridge.deliver_signals()
                    value=None
                    if method == "runner_function":
                        member=self.module
                        for part in p[0].split("."): member=getattr(member,part)
                        values["function"]=member
                    elif method == "runner_invoke": invoke(p)
                    elif method == "provider_literal": values[p[0]]=p[1]
                    elif method == "provider_copy": values[p[0]]=values[p[1]]
                    elif method == "provider_bool": value=bool(values[p[0]])
                    elif method == "provider_is_none": value=values[p[0]] is None
                    elif method == "provider_equal": value=bool(values[p[0]] == p[1])
                    elif method == "provider_contains": value=values[p[0]] in values[p[1]]
                    elif method == "provider_tuple": values["tuple"]=tuple(values[key] for key in p)
                    elif method == "stdout_member": values[p[2]]=getattr(values[p[0]],p[1])
                    elif method == "stream_set": setattr(values["self"],p[0],values[p[1]])
                    elif method == "eval_builtin": values["function"]=getattr(builtins,p[0])
                    elif method in ("scaffold_pair","workspace_pair"): values[p[1]],values[p[2]]=values[p[0]]
                    elif method == "stream_item": values[p[2]]=values[p[0]][p[1]]
                    elif method == "scaffold_item": values[p[2]]=values[p[0]][values[p[1]]]
                    elif method == "eval_field": values[p[0]][p[1]]=values[p[2]]
                    elif method == "workspace_put": values[p[0]][values[p[1]]]=values[p[2]]
                    elif method == "workspace_iter": values[p[1]]=iter(values[p[0]])
                    elif method == "workspace_next":
                        try: values[p[1]]=next(values[p[0]])
                        except StopIteration: value=False
                        else: value=True
                    elif method == "workspace_set": values[p[0]]=set()
                    elif method == "workspace_slice": values[p[2]]=values[p[0]][:values[p[1]]]
                    elif method == "workspace_local":
                        if p[0] not in values: raise UnboundLocalError(f"cannot access local variable '{p[0]}' where it is not associated with a value")
                        values[p[1]]=values[p[0]]
                    elif method == "eval_append": values[p[0]].append(values[p[1]])
                    elif method == "web_compare": values["comparison"]=getattr(operator,p[0])(values[p[1]],values[p[2]])
                    elif method == "web_binary": values[p[3]]=getattr(operator,p[0])(values[p[1]],values[p[2]])
                    elif method == "logger_literal": values[p[0]]="".join(map(chr,p[1]))
                    elif method == "task_format": value=list(map(ord,repr(values[p[0]]) if p[1] == "!r" else format(values[p[0]],p[1])))
                    elif method == "driver_env_lines": value=[list(map(ord,line)) for line in values["text"].splitlines()]
                    elif method == "driver_argument": values["ap"].add_argument(p[0],**p[1],**{key:values[slot] for key,slot in p[2]})
                    elif method == "driver_adapters": values.update(self._adapters())
                    elif method == "task_raise": raise values[p[0]]
                    elif method == "task_call_kwargs": invoke(p,extra=p[2])
                    elif method == "task_enter":
                        values[p[0]+"_exit"]=special(values["called"],"__exit__")
                        values[p[0]+"_handle"]=special(values["called"],"__enter__")()
                    elif method == "task_exit":
                        error=error_object(p[1]) if p[1] is not None else None
                        arguments=(type(error),error,BaseException.__traceback__.__get__(error)) if error is not None else (None,None,None)
                        values["exit_result"]=in_exception(error,lambda:bool(values[p[0]+"_exit"](*arguments)))
                    elif method == "handler_exception": values["caught_error"]=error_object(p[0]) if p[0] is not None else None
                    elif method == "handler_member": values[p[2]]=in_exception(values.get("caught_error"),getattr,values[p[0]],p[1])
                    elif method == "handler_invoke": invoke(p,handled=True)
                    elif method == "exception_matches": value=isinstance(error_object(p[0]),tuple(getattr(builtins,kind) for kind in p[1]))
                    else: raise ValueError("unsupported driver callback: "+method)
                    reply={"value":value}
            except BaseException as error:
                errors.append(error)
                reply={"error":{"kind":type(error).__name__,"message":"","details":{"provider_error_index":len(errors)-1},"bases":[kind.__module__+"."+kind.__qualname__ for kind in type(error).__mro__]}}
            result=self.bridge.request("experiment.swebench.resume",{"job":result["job"],"reply":reply})
        if isinstance(result,dict) and "raised" in result:
            if "provider_error_index" in (result.get("details") or {}): raise error_object(result)
            checked(result)
        self.bridge.deliver_signals()
        if isinstance(result,dict) and "provider_ref" in result: return values[result["provider_ref"]]
        if isinstance(result,dict) and "provider_encoded" in result: return json.loads(result["provider_encoded"])
        return result
