"""Task lifecycle object and I/O bindings; task policy executes in Rust."""
import functools
import json
import os
import inspect
import sys
import subprocess
import math

from .transport import checked, data
from .swebench_guard import bind as guard_bindings

NO_COMMAND=object()


def bind_network(bridge,module):
    invoke=bridge.request
    original = module.network_tool_block
    @functools.wraps(original)
    def native():
        with bridge.frames():
            env = {key:os.environ[key] for key in ("TE_ALLOW_NETWORK_TOOLS",) if key in os.environ}
            result=tuple(checked(invoke("experiment.swebench.network_tool_block",{"env":env})))
            bridge.deliver_signals()
            return result
    module.network_tool_block = native
    return {original:native}


def bind(bridge,module,*,container_helpers=lambda:None):
    """Bind the supplied harness; container_helpers resolves its current helper directory."""
    invoke=bridge.request
    def wraps(original):
        def decorate(callback):
            @functools.wraps(original)
            def native(*args,**kwargs):
                with bridge.frames():
                    result=callback(*args,**kwargs)
                    bridge.deliver_signals()
                    return result
            return native
        return decorate
    cls = module.ClaudeSwebenchRunner
    replacements = {}
    original_attempts = module.answer_key_attempts
    @wraps(original_attempts)
    def answer_key_attempts(run_dir_path):
        return checked(invoke("experiment.swebench.answer_key_attempts",{"run_dir_path":os.fspath(run_dir_path),"process_cwd":os.getcwd()}))
    module.answer_key_attempts = answer_key_attempts
    replacements[original_attempts] = answer_key_attempts
    def config_value(value, representation):
        if representation == "bool": return bool(value)
        if representation == "present": return value is not None
        if representation == "list": return list(value)
        # shortcut: object strings are captured per callback; audit conversion side effects before release.
        return json.loads(json.dumps(value,default=str))
    def drive(self, result, run_kwargs, guard=None, process_cmd=NO_COMMAND, record_row=None, task_call=None, control_prompt=NO_COMMAND):
        errors = []
        processes = {}
        transcript = None
        spec = None
        scaffold_result = None
        popped = None
        task_result = None
        diff_excludes = None
        control_values = {"prompt":control_prompt}
        control_result,control_telemetry,control_snapshot = None,None,None
        control_row = {}
        while isinstance(result,dict) and "callback" in result:
            method, args = result["method"], result["args"]
            try:
                with bridge.callback_scope():
                    bridge.deliver_signals()
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
                    elif method == "control_capture":
                        control_values[args[0]] = run_kwargs.get(*args)
                        value = None
                    elif method == "control_bool": value = bool(control_values[args[0]])
                    elif method == "control_env": value = module.os.environ.get(*args)
                    elif method == "control_present": value = control_values[args[0]] is not None
                    elif method == "control_integer_input":
                        item = control_values[args[0]]
                        kind = type(item).__name__
                        external = type(item) not in (type(None),bool,int,float,str,list,dict,tuple) or type(item) is float and not math.isfinite(item)
                        value = {"kind":"external" if external else kind,"value":None if external or type(item) is int else data(item)}
                    elif method == "control_external_int":
                        control_values[args[0]] = int(control_values[args[0]])
                        value = None
                    elif method == "control_signature": value = list(module.inspect.signature(module.NudgeGuard.__init__).parameters)
                    elif method == "control_new":
                        parameters = {name:control_values[item] if representation == "ref" else int(item) if representation == "integer" else item for name,representation,item in args[1]}
                        constructor = module.NudgeGuard if args[0] == "nudge" else module.codex_spiral.CodexResumeGuard
                        control_result = constructor(**parameters)
                        value = None
                    elif method == "control_continue":
                        control_result = guard.continuation(*[control_values[key] for key in args])
                        value = None
                    elif method == "control_telemetry":
                        control_telemetry = guard.telemetry()
                        value = None
                    elif method == "control_snapshot":
                        control_snapshot = control_telemetry.get(*args)
                        value = bool(control_snapshot)
                    elif method == "control_snapshot_get":
                        control_row[args[0]] = control_snapshot.get(args[1])
                        value = None
                    elif method == "control_row_value":
                        control_row[args[0]] = args[1]
                        value = None
                    elif method == "control_row_items":
                        for name,key in args: control_row[name] = control_telemetry[key]
                        value = None
                    elif method == "control_row_attributes":
                        for name,attribute,*default in args: control_row[name] = getattr(guard,attribute,*default)
                        value = None
                    elif method == "_guard_continuation": value = self._guard_continuation(guard,*args)
                    elif method == "_guard_row": value = self._guard_row(guard)
                    elif method == "_write_run_config": value = self._write_run_config(*args,run_kwargs,guard)
                    elif method == "config_attr":
                        owner, name, representation, *default = args
                        target = self if owner == "runner" else guard if owner == "guard" else getattr(self,owner)
                        value = config_value(getattr(target,name,*default),representation)
                    elif method == "config_kwarg": value = config_value(run_kwargs.get(args[0]),args[1])
                    elif method == "config_set":
                        setattr(self,args[0],args[1])
                        value = None
                    elif method == "runner_capabilities":
                        self._capabilities = tuple(args)
                        value = None
                    elif method == "runner_base_task":
                        task_result = task_call()
                        value = {"out_dir":os.fspath(task_result[1]),"cwd":os.getcwd()}
                    elif method == "runner_row_get": value = task_result[0].get(*args)
                    elif method == "runner_row_pop": value = task_result[0].pop(*args)
                    elif method == "runner_row_update":
                        task_result[0].update(*args)
                        value = None
                    elif method == "config_function": value = getattr(module,args[0])(*args[1])
                    elif method == "net_guard_function": value = getattr(module.net_guard,args[0])(*args[1])
                    elif method == "net_guard_constant": value = getattr(module.net_guard,args[0])
                    elif method == "codex_function": value = getattr(module.codex,args[0])(*args[1],**args[2])
                    elif method == "codex_constant": value = getattr(module.codex,args[0])
                    elif method == "scaffold_setup":
                        scaffold_result = (self.scaffold.setup(self._scaffold_context(*args)),None)
                        value = None
                    elif method == "scaffold_resolve_mcp":
                        scaffold_result = (scaffold_result[0],scaffold_result[0].get(*args))
                        value = None
                    elif method == "base_scaffold":
                        scaffold_result = module.ClaudeSwebenchRunner._apply_scaffold(self,*args)
                        value = {"is_dict":isinstance(scaffold_result[0],dict)}
                    elif method == "scaffold_notes_pop":
                        popped = scaffold_result[0].pop(*args)
                        value = popped
                    elif method == "scaffold_use_popped":
                        setattr(self,args[0],popped)
                        value = None
                    elif method == "scaffold_notes_update":
                        scaffold_result[0].update(args[0])
                        value = None
                    elif method == "config_patch_excludes": value = list(self._patch_excludes())
                    elif method == "guard_patch_excludes":
                        diff_excludes = self._patch_excludes()
                        value = bool(diff_excludes)
                    elif method == "guard_exclude_values": value = list(diff_excludes)
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
        if isinstance(result,dict) and result.get("_scaffold_result"): return scaffold_result
        if isinstance(result,dict) and result.get("_runner_result"): return task_result[0],task_result[1]
        if isinstance(result,dict) and result.get("_control_result"): return control_result
        if isinstance(result,dict) and result.get("_control_row"): return control_row
        return result
    def codex_function(original,operation):
        signature = inspect.signature(original)
        @wraps(original)
        def native(*args,**kwargs):
            bound = signature.bind(*args,**kwargs)
            bound.apply_defaults()
            arguments = data(dict(bound.arguments))
            arguments["environment"] = {key:os.environ[key] for key in ("TE_ALLOW_NETWORK_TOOLS",) if key in os.environ}
            arguments["process_cwd"] = os.getcwd()
            value = drive(None,invoke("experiment.swebench.codex."+operation,arguments),{})
            if operation == "network_block_toggles": return tuple(value[0]),tuple(tuple(pair) for pair in value[1])
            if operation == "config_toggles": return value[0],[tuple(pair) for pair in value[1]]
            return value
        replacements[original] = native
        return native
    for name in ("network_block_toggles","config_toggles","config_toml","skills_config_entries","lean_instructions","_unwrap_shell","rtk_compliance","mcp_tool_calls"):
        setattr(module.codex,name,codex_function(getattr(module.codex,name),name))
    original_write_config = module.CodexSwebenchRunner._write_config
    @wraps(original_write_config)
    def write_config(self,name):
        return drive(self,invoke("experiment.swebench.codex.write_config",{"name":name}),{})
    module.CodexSwebenchRunner._write_config = write_config
    def scaffold_method(original,operation):
        @wraps(original)
        def native(self,name,task,run_dir_path):
            return tuple(drive(self,invoke("experiment.swebench.codex."+operation,{"name":name,"task":data(task),"run_dir_path":os.fspath(run_dir_path)}),{}))
        return native
    cls._apply_scaffold = scaffold_method(cls._apply_scaffold,"apply_scaffold")
    module.CodexSwebenchRunner._apply_scaffold = scaffold_method(module.CodexSwebenchRunner._apply_scaffold,"apply_codex_scaffold")
    original_run = module._run
    @wraps(original_run)
    def run(cmd,timeout=120,check=False):
        return drive(None,invoke("experiment.swebench.container.run",{"cmd":data(cmd),"timeout":timeout,"check":check}),{},process_cmd=cmd)
    module._run = run
    replacements[original_run] = run
    def container_method(original,operation):
        signature = inspect.signature(original)
        @wraps(original)
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
    @wraps(original_activate)
    def activate(self,name):
        drive(self,invoke("experiment.swebench.container.activate",{"name":name}),{})
        return self.__dict__["_env_activation"][name]
    cls._activate_project_env = activate
    original_info = cls._env_activation_info
    @wraps(original_info)
    def activation_info(self,name):
        selected = drive(self,invoke("experiment.swebench.container.activation_info",{"state":data(self.__dict__.get("_env_activation",{})),"name":name}),{})
        return self.__dict__["_env_activation"][name] if selected["stored"] else selected["info"]
    cls._env_activation_info = activation_info
    original_record = cls._record_env_activation
    @wraps(original_record)
    def record_activation(self,run_dir_path,name,row):
        return drive(self,invoke("experiment.swebench.container.record_activation",{"run_dir":os.fspath(run_dir_path),"name":name,"process_cwd":os.getcwd()}),{},record_row=row)
    cls._record_env_activation = record_activation
    def start_method(original,agent):
        @wraps(original)
        def native(self,name,instance_id):
            return drive(self,invoke("experiment.swebench.container.start",{"agent":agent,"name":name,"instance_id":instance_id}),{})
        return native
    for klass,agent in ((cls,"claude"),(module.CopilotSwebenchRunner,"copilot"),(module.CodexSwebenchRunner,"codex")):
        klass._start = start_method(klass._start,agent)
    def container_function(original,operation):
        signature = inspect.signature(original)
        @wraps(original)
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
    @wraps(original)
    def run_task(self, task, *, arm, injected, exclude_tools, run_dir_path, **run_kwargs):
        result = invoke("experiment.swebench.run_task", {"task":data(task), "arm":arm, "injected":injected,
                        "exclude_tools":exclude_tools, "run_dir":os.fspath(run_dir_path), "kwargs":data(run_kwargs),
                        "metadata":{key:getattr(self,key) for key in ("agent","model","effort","cli_version")},
                        "environment":{key:os.environ[key] for key in ("TE_MAX_TURNS",) if key in os.environ}, "process_cwd":os.getcwd()})
        return drive(self,result,run_kwargs), run_dir_path
    cls.run_task = run_task
    original_accepts = module.guard_accepts
    @wraps(original_accepts)
    def guard_accepts(param):
        return drive(None,invoke("experiment.swebench.control.accepts",{"param":data(param)}),{})
    module.guard_accepts = guard_accepts
    replacements[original_accepts] = guard_accepts
    def control_methods(klass,agent):
        original_build,original_row,original_continuation = klass._build_guard,klass._guard_row,klass._guard_continuation
        @wraps(original_build)
        def build(self,run_kwargs):
            return drive(self,invoke("experiment.swebench.control.build",{"agent":agent,"int_max_digits":getattr(sys,"get_int_max_str_digits",lambda:0)()}),run_kwargs)
        @wraps(original_row)
        def row(self,guard):
            return drive(self,invoke("experiment.swebench.control.row",{"agent":agent}),{},guard)
        @wraps(original_continuation)
        def continuation(self,guard,prompt):
            return drive(self,invoke("experiment.swebench.control.continuation",{"agent":agent}),{},guard,control_prompt=prompt)
        klass._build_guard,klass._guard_row,klass._guard_continuation = build,row,continuation
    control_methods(cls,"claude")
    control_methods(module.CodexSwebenchRunner,"codex")
    def guard_method(original,operation):
        signature = inspect.signature(original)
        @wraps(original)
        def native(*args,**kwargs):
            bound = signature.bind(*args,**kwargs)
            bound.apply_defaults()
            arguments = dict(bound.arguments)
            self = arguments.pop("self")
            if operation == "blocks": arguments["int_max_digits"] = getattr(sys,"get_int_max_str_digits",lambda:0)()
            if operation == "install": arguments["helper_root"] = container_helpers()
            value = drive(self,invoke("experiment.swebench.guard."+operation,data(arguments)),{})
            return int(value) if operation == "blocks" else value
        return native
    for name,operation in (("_install_net_guard","install"),("_install_answer_key_hosts","hosts"),("_verify_net_guard","verify"),("_net_guard_blocks","blocks"),("_graded_diff","graded_diff")):
        setattr(cls,name,guard_method(getattr(cls,name),operation))
    def runner_method(original,agent):
        @wraps(original)
        def native(self,task,*,arm,injected,exclude_tools,run_dir_path,**run_kwargs):
            def task_call():
                return cls.run_task(self,task,arm=arm,injected=injected,exclude_tools=exclude_tools,run_dir_path=run_dir_path,**run_kwargs)
            result = invoke("experiment.swebench.runner."+agent,{"task":data(task)})
            return drive(self,result,run_kwargs,task_call=task_call)
        return native
    for klass,agent in ((module.CopilotSwebenchRunner,"copilot"),(module.CodexSwebenchRunner,"codex")):
        klass.run_task = runner_method(klass.run_task,agent)
    original_config = cls._write_run_config
    @wraps(original_config)
    def write_run_config(self,run_dir_path,task,arm,injected,exclude_tools,max_turns,run_kwargs,guard):
        result = invoke("experiment.swebench.write_run_config",{"run_dir":os.fspath(run_dir_path),"task":data(task),"arm":arm,
                        "injected":injected,"exclude_tools":data(exclude_tools),"max_turns":max_turns,"process_cwd":os.getcwd()})
        return drive(self,result,run_kwargs,guard)
    cls._write_run_config = write_run_config
    original_excludes = cls._patch_excludes
    @wraps(original_excludes)
    def patch_excludes(self):
        return tuple(drive(self,invoke("experiment.swebench.patch_excludes",{}),{}))
    cls._patch_excludes = patch_excludes
    def helper(original,operation):
        signature = inspect.signature(original)
        @wraps(original)
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
    original_grade = cls._grade
    @wraps(original_grade)
    def grade(self, name, instance_id, patch, run_dir_path):
        result = invoke("experiment.swebench.grade", {"name":name,"instance_id":instance_id,"patch":patch,
                        "run_dir":os.fspath(run_dir_path),"eval_timeout":self.eval_timeout,"process_cwd":os.getcwd()})
        return tuple(drive(self,result,{}))
    cls._grade = grade
    original_failed = cls._eval_exec_failed
    @wraps(original_failed)
    def failed(self, instance_id, host_log, out, rc, stderr, phase):
        result = invoke("experiment.swebench.eval_exec_failed", {"instance_id":instance_id,"host_log":os.fspath(host_log),
                        "out":out,"rc":rc,"stderr":stderr,"phase":phase,"process_cwd":os.getcwd()})
        return drive(self,result,{})
    cls._eval_exec_failed = failed
    def environment():
        return {key:os.environ[key] for key in ("TE_SWEBENCH_GRADING","DATA","TE_SWEBENCH_REPO") if key in os.environ}
    original_default = module.default_grading_path
    @wraps(original_default)
    def default_grading_path():
        return checked(invoke("experiment.swebench.default_grading_path",{"env":environment()}))
    module.default_grading_path = default_grading_path
    original_repo = module.swebench_repo
    @wraps(original_repo)
    def swebench_repo():
        return checked(invoke("experiment.swebench.swebench_repo",{"env":environment(),"process_cwd":os.getcwd()}))
    module.swebench_repo = swebench_repo
    grader = module._Grader
    original_init = grader.__init__
    @wraps(original_init)
    def grader_init(self,grading_path=None):
        selected = checked(invoke("experiment.swebench.grader.path",{"grading_path":data(grading_path),"env":environment()}))
        self.path = grading_path if selected["explicit"] else selected["path"]
        self.rows = checked(invoke("experiment.swebench.grader.load",{"path":os.fspath(self.path),"process_cwd":os.getcwd()}))
        self.make_test_spec,self.get_eval_report = module._import_swebench()
    grader.__init__ = grader_init
    original_instance = grader.instance
    @wraps(original_instance)
    def instance(self,instance_id):
        return checked(invoke("experiment.swebench.grader.instance",{"self":{"path":os.fspath(self.path),"rows":self.rows},"instance_id":instance_id}))
    grader.instance = instance
    original_eval = grader.eval_script
    @wraps(original_eval)
    def eval_script(self,instance_id):
        return drive(self,invoke("experiment.swebench.grader.eval_script",{"instance_id":instance_id}),{})
    grader.eval_script = eval_script
    original_score = grader.score
    @wraps(original_score)
    def score(self,instance_id,model_patch,log_path):
        return tuple(drive(self,invoke("experiment.swebench.grader.score",{"instance_id":instance_id,"model_patch":model_patch,"log_path":os.fspath(log_path)}),{}))
    grader.score = score
    original_name = cls._container_name
    @wraps(original_name)
    def container_name(self, instance_id, arm, run_dir_path):
        return checked(invoke("experiment.swebench.container_name", {"instance_id":instance_id,"arm":arm,"agent":self.agent,
                       "model":self.model,"run_dir_path":os.fspath(run_dir_path),"process_cwd":os.getcwd()}))
    cls._container_name = container_name
    original_prompt = module.build_prompt
    @wraps(original_prompt)
    def build_prompt(task,injected):
        return checked(invoke("experiment.swebench.build_prompt", {"task":task,"injected":injected}))
    module.build_prompt = build_prompt
    for klass, agent in ((cls,"claude"), (module.CopilotSwebenchRunner,"copilot"), (module.CodexSwebenchRunner,"codex")):
        original_row = klass._row_from_segment
        def row_method(original, domain):
            @wraps(original)
            def native(self,result):
                return checked(invoke("experiment.swebench.row_from_segment", {"agent":domain,"result":result}))
            return native
        klass._row_from_segment = row_method(original_row,agent)
    replacements.update(guard_bindings(bridge,module))
    return replacements
