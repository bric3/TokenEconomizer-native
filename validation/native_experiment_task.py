"""I/O callbacks for the native SWE-bench task lifecycle."""
import dataclasses
import functools
import json
import os
import inspect
import sys
import subprocess

from native_plugin import invoke
from native_experiment_spiral import checked as checked_value

NO_COMMAND = object()


def checked(value):
    if isinstance(value,dict) and "errno" in value.get("details",{}):
        info = value["details"]
        raise OSError(info["errno"],info["strerror"],info["filename"])
    if isinstance(value,dict) and value.get("raised") == "JSONDecodeError":
        raise json.JSONDecodeError(value["message"],value["details"]["doc"],value["details"]["pos"])
    if isinstance(value,dict) and value.get("raised") == "UnicodeDecodeError":
        info = value["details"]
        raise UnicodeDecodeError("utf-8",bytes(info["bytes"]),info["start"],info["end"],info["reason"])
    return checked_value(value)


def data(value):
    if isinstance(value, bytes): return {"bytes":list(value)}
    if dataclasses.is_dataclass(value): return dataclasses.asdict(value)
    if isinstance(value, os.PathLike): return os.fspath(value)
    if isinstance(value, dict): return {key: data(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)): return [data(item) for item in value]
    if callable(value): return True
    return value


def install_network(module):
    original = module.network_tool_block
    @functools.wraps(original)
    def native():
        env = {key:os.environ[key] for key in ("TE_ALLOW_NETWORK_TOOLS",) if key in os.environ}
        return tuple(checked(invoke("experiment.swebench.network_tool_block",{"env":env})))
    module.network_tool_block = native
    return {original:native}


def install():
    from te_harness import swebench as module
    cls = module.ClaudeSwebenchRunner
    replacements = {}
    def config_value(value, representation):
        if representation == "bool": return bool(value)
        if representation == "present": return value is not None
        if representation == "list": return list(value)
        # shortcut: object strings are captured per callback; audit conversion side effects before release.
        return json.loads(json.dumps(value,default=str))
    def drive(self, result, run_kwargs, guard=None, process_cmd=NO_COMMAND, record_row=None):
        errors = []
        processes = {}
        transcript = None
        spec = None
        while isinstance(result,dict) and "callback" in result:
            method, args = result["method"], result["args"]
            try:
                if method == "clock": value = module.time.time()
                elif method == "log": value = module._log(*args)
                elif method == "optimizer_context":
                    module._OPT_CTX.clear()
                    module._OPT_CTX.update(args[0])
                    value = None
                elif method == "_build_guard":
                    guard = self._build_guard(run_kwargs)
                    value = guard is not None
                elif method == "guard_segment": value = guard.segment
                elif method == "_guard_continuation": value = self._guard_continuation(guard,*args)
                elif method == "_guard_row": value = self._guard_row(guard)
                elif method == "_write_run_config": value = self._write_run_config(*args,run_kwargs,guard)
                elif method == "config_attr":
                    owner, name, representation, *default = args
                    target = self if owner == "runner" else guard if owner == "guard" else getattr(self,owner)
                    value = config_value(getattr(target,name,*default),representation)
                elif method == "config_kwarg": value = config_value(run_kwargs.get(args[0]),args[1])
                elif method == "config_function": value = getattr(module,args[0])(*args[1])
                elif method == "config_patch_excludes": value = list(self._patch_excludes())
                elif method in ("container_run","subprocess_run"):
                    command, timeout = args
                    if method == "container_run": process = module._run(command,timeout=timeout)
                    else:
                        command = process_cmd if process_cmd is not NO_COMMAND else command
                        process = subprocess.run(command,capture_output=True,text=True,timeout=timeout)
                    reference = len(processes)
                    processes[reference] = process
                    value = {"_process_ref":reference,"command":data(command),**{key:getattr(process,key,None) for key in ("returncode","stdout","stderr")}}
                elif method == "container_env": value = os.environ.get(*args)
                elif method == "live_add": value = module._LIVE_CONTAINERS.add(*args)
                elif method == "live_discard": value = module._LIVE_CONTAINERS.discard(*args)
                elif method == "activation_ensure":
                    self.__dict__.setdefault("_env_activation",{})
                    value = None
                elif method == "activation_put":
                    self.__dict__["_env_activation"][args[0]] = args[1]
                    value = None
                elif method == "activation_row":
                    record_row[args[0]] = args[1]
                    value = None
                elif method == "scaffold_mounts": value = list(self.scaffold.mounts(self.scaffold_profile))
                elif method == "agent_mounts": value = getattr(module,args[0]).container_mounts(args[1])
                elif method == "copilot_function": value = getattr(module.copilot,args[0])(*args[1])
                elif method == "expanduser": value = os.path.expanduser(*args)
                elif method == "_record_env_activation":
                    row = args[2]
                    self._record_env_activation(args[0],args[1],row)
                    value = row
                elif method == "transcript_open":
                    transcript = open(args[0], "w", encoding="utf-8")
                    value = None
                elif method == "transcript_close":
                    transcript.close()
                    transcript = None
                    value = None
                elif method == "_segment":
                    name, prompt, path, excluded, turns, mcp = args
                    value = self._segment(name,prompt,transcript,excluded,turns,guard,mcp_config=mcp)
                elif method == "_exec":
                    process = self._exec(args[0],args[1],**args[2])
                    value = {key:getattr(process,key) for key in ("returncode","stdout","stderr")}
                elif method == "eval_script": value = self.grader.eval_script(*args)
                elif method == "score": value = self.grader.score(*args)
                elif method == "grader_instance": value = self.instance(*args)
                elif method == "make_test_spec":
                    spec = self.make_test_spec(*args)
                    value = None
                elif method == "eval_script_value": value = spec.eval_script
                elif method == "get_eval_report":
                    value = self.get_eval_report(test_spec=spec,prediction=args[0],test_log_path=args[1],include_tests_status=args[2])
                elif method == "scaffold_info": value = {"name":self.scaffold.name} if self.scaffold is not None else None
                else: value = getattr(self,method)(*args)
                reply = {"value":data(value)}
            except BaseException as error:
                errors.append(error)
                reply = {"error":{"kind":type(error).__name__,"message":str(error),"bases":[base.__module__ + "." + base.__qualname__ for base in type(error).__mro__],"stdout":data(getattr(error,"stdout",None))}}
            result = invoke("experiment.swebench.resume", {"job":result["job"],"reply":reply})
        if isinstance(result,dict) and "raised" in result:
            for error in reversed(errors):
                if type(error).__name__ == result["raised"] and str(error) == result["message"]:
                    raise error
            checked(result)
        if isinstance(result,dict) and "_process_ref" in result: return processes[result["_process_ref"]]
        return result
    original_run = module._run
    @functools.wraps(original_run)
    def run(cmd,timeout=120,check=False):
        return drive(None,invoke("experiment.swebench.container.run",{"cmd":data(cmd),"timeout":timeout,"check":check}),{},process_cmd=cmd)
    module._run = run
    replacements[original_run] = run
    def container_method(original,operation):
        signature = inspect.signature(original)
        @functools.wraps(original)
        def native(self,*args,**kwargs):
            bound = signature.bind(self,*args,**kwargs)
            bound.apply_defaults()
            arguments = dict(bound.arguments)
            arguments.pop("self")
            return drive(self,invoke("experiment.swebench.container."+operation,data(arguments)),{})
        return native
    for name,operation in (("_reap_exited","reap"),("_stop","stop"),("_exec","exec"),("_env_activation_args","activation_args")):
        setattr(cls,name,container_method(getattr(cls,name),operation))
    original_activate = cls._activate_project_env
    @functools.wraps(original_activate)
    def activate(self,name):
        drive(self,invoke("experiment.swebench.container.activate",{"name":name}),{})
        return self.__dict__["_env_activation"][name]
    cls._activate_project_env = activate
    original_info = cls._env_activation_info
    @functools.wraps(original_info)
    def activation_info(self,name):
        selected = drive(self,invoke("experiment.swebench.container.activation_info",{"state":data(self.__dict__.get("_env_activation",{})),"name":name}),{})
        return self.__dict__["_env_activation"][name] if selected["stored"] else selected["info"]
    cls._env_activation_info = activation_info
    original_record = cls._record_env_activation
    @functools.wraps(original_record)
    def record_activation(self,run_dir_path,name,row):
        return drive(self,invoke("experiment.swebench.container.record_activation",{"run_dir":os.fspath(run_dir_path),"name":name,"process_cwd":os.getcwd()}),{},record_row=row)
    cls._record_env_activation = record_activation
    def start_method(original,agent):
        @functools.wraps(original)
        def native(self,name,instance_id):
            return drive(self,invoke("experiment.swebench.container.start",{"agent":agent,"name":name,"instance_id":instance_id}),{})
        return native
    for klass,agent in ((cls,"claude"),(module.CopilotSwebenchRunner,"copilot"),(module.CodexSwebenchRunner,"codex")):
        klass._start = start_method(klass._start,agent)
    def container_function(original,operation):
        signature = inspect.signature(original)
        @functools.wraps(original)
        def native(*args,**kwargs):
            bound = signature.bind(*args,**kwargs)
            bound.apply_defaults()
            return drive(None,invoke("experiment.swebench.container."+operation,data(dict(bound.arguments))),{})
        replacements[original] = native
        return native
    for name,operation in (("copilot_cache","copilot_cache"),("copilot_config","copilot_config"),("container_mounts","copilot_mounts"),("container_seed_setup","copilot_seed")):
        setattr(module.copilot,name,container_function(getattr(module.copilot,name),operation))
    module.codex.container_mounts = container_function(module.codex.container_mounts,"codex_mounts")
    original = cls.run_task
    @functools.wraps(original)
    def run_task(self, task, *, arm, injected, exclude_tools, run_dir_path, **run_kwargs):
        result = invoke("experiment.swebench.run_task", {"task":data(task), "arm":arm, "injected":injected,
                        "exclude_tools":exclude_tools, "run_dir":os.fspath(run_dir_path), "kwargs":data(run_kwargs),
                        "metadata":{key:getattr(self,key) for key in ("agent","model","effort","cli_version")},
                        "environment":{key:os.environ[key] for key in ("TE_MAX_TURNS",) if key in os.environ}, "process_cwd":os.getcwd()})
        return drive(self,result,run_kwargs), run_dir_path
    cls.run_task = run_task
    original_config = cls._write_run_config
    @functools.wraps(original_config)
    def write_run_config(self,run_dir_path,task,arm,injected,exclude_tools,max_turns,run_kwargs,guard):
        result = invoke("experiment.swebench.write_run_config",{"run_dir":os.fspath(run_dir_path),"task":data(task),"arm":arm,
                        "injected":injected,"exclude_tools":data(exclude_tools),"max_turns":max_turns,"process_cwd":os.getcwd()})
        return drive(self,result,run_kwargs,guard)
    cls._write_run_config = write_run_config
    original_excludes = cls._patch_excludes
    @functools.wraps(original_excludes)
    def patch_excludes(self):
        return tuple(drive(self,invoke("experiment.swebench.patch_excludes",{}),{}))
    cls._patch_excludes = patch_excludes
    def helper(original,operation):
        signature = inspect.signature(original)
        @functools.wraps(original)
        def native(*args,**kwargs):
            bound = signature.bind(*args,**kwargs)
            bound.apply_defaults()
            arguments = data(dict(bound.arguments))
            arguments["env"] = {key:os.environ[key] for key in ("TE_ENV_ACTIVATION","TE_ALLOW_NETWORK_TOOLS","TE_SWEBENCH_MAX_CPUS") if key in os.environ}
            value = checked(invoke("experiment.swebench."+operation,arguments))
            return tuple(value) if operation == "network_tool_block" else value
        replacements[original] = native
        return native
    for name in ("env_activation_enabled","network_tool_block","_cpu_limit_args","choose_project_env","activation_env","image_for"):
        setattr(module,name,helper(getattr(module,name),name))
    webbench = sys.modules.get("te_harness.webbench")
    if webbench is not None and hasattr(webbench,"network_tool_block"):
        replacements.update(install_network(webbench))
    original_grade = cls._grade
    @functools.wraps(original_grade)
    def grade(self, name, instance_id, patch, run_dir_path):
        result = invoke("experiment.swebench.grade", {"name":name,"instance_id":instance_id,"patch":patch,
                        "run_dir":os.fspath(run_dir_path),"eval_timeout":self.eval_timeout,"process_cwd":os.getcwd()})
        return tuple(drive(self,result,{}))
    cls._grade = grade
    original_failed = cls._eval_exec_failed
    @functools.wraps(original_failed)
    def failed(self, instance_id, host_log, out, rc, stderr, phase):
        result = invoke("experiment.swebench.eval_exec_failed", {"instance_id":instance_id,"host_log":os.fspath(host_log),
                        "out":out,"rc":rc,"stderr":stderr,"phase":phase,"process_cwd":os.getcwd()})
        return drive(self,result,{})
    cls._eval_exec_failed = failed
    def environment():
        return {key:os.environ[key] for key in ("TE_SWEBENCH_GRADING","DATA","TE_SWEBENCH_REPO") if key in os.environ}
    original_default = module.default_grading_path
    @functools.wraps(original_default)
    def default_grading_path():
        return checked(invoke("experiment.swebench.default_grading_path",{"env":environment()}))
    module.default_grading_path = default_grading_path
    original_repo = module.swebench_repo
    @functools.wraps(original_repo)
    def swebench_repo():
        return checked(invoke("experiment.swebench.swebench_repo",{"env":environment(),"process_cwd":os.getcwd()}))
    module.swebench_repo = swebench_repo
    grader = module._Grader
    original_init = grader.__init__
    @functools.wraps(original_init)
    def grader_init(self,grading_path=None):
        selected = checked(invoke("experiment.swebench.grader.path",{"grading_path":data(grading_path),"env":environment()}))
        self.path = grading_path if selected["explicit"] else selected["path"]
        self.rows = checked(invoke("experiment.swebench.grader.load",{"path":os.fspath(self.path),"process_cwd":os.getcwd()}))
        self.make_test_spec,self.get_eval_report = module._import_swebench()
    grader.__init__ = grader_init
    original_instance = grader.instance
    @functools.wraps(original_instance)
    def instance(self,instance_id):
        return checked(invoke("experiment.swebench.grader.instance",{"self":{"path":os.fspath(self.path),"rows":self.rows},"instance_id":instance_id}))
    grader.instance = instance
    original_eval = grader.eval_script
    @functools.wraps(original_eval)
    def eval_script(self,instance_id):
        return drive(self,invoke("experiment.swebench.grader.eval_script",{"instance_id":instance_id}),{})
    grader.eval_script = eval_script
    original_score = grader.score
    @functools.wraps(original_score)
    def score(self,instance_id,model_patch,log_path):
        return tuple(drive(self,invoke("experiment.swebench.grader.score",{"instance_id":instance_id,"model_patch":model_patch,"log_path":os.fspath(log_path)}),{}))
    grader.score = score
    original_name = cls._container_name
    @functools.wraps(original_name)
    def container_name(self, instance_id, arm, run_dir_path):
        return checked(invoke("experiment.swebench.container_name", {"instance_id":instance_id,"arm":arm,"agent":self.agent,
                       "model":self.model,"run_dir_path":os.fspath(run_dir_path),"process_cwd":os.getcwd()}))
    cls._container_name = container_name
    original_prompt = module.build_prompt
    @functools.wraps(original_prompt)
    def build_prompt(task,injected):
        return checked(invoke("experiment.swebench.build_prompt", {"task":task,"injected":injected}))
    module.build_prompt = build_prompt
    for klass, agent in ((cls,"claude"), (module.CopilotSwebenchRunner,"copilot"), (module.CodexSwebenchRunner,"codex")):
        original_row = klass._row_from_segment
        def row_method(original, domain):
            @functools.wraps(original)
            def native(self,result):
                return checked(invoke("experiment.swebench.row_from_segment", {"agent":domain,"result":result}))
            return native
        klass._row_from_segment = row_method(original_row,agent)
    return replacements
