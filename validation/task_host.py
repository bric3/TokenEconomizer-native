"""Local container, stream and grader stand-ins for task-lifecycle comparisons."""
import builtins
import contextlib
import io
import json
import os
import subprocess
import signal
import sys
import types
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if "--sdk" in sys.argv:
    from standalone_harness import sw,wb
else:
    sys.path[:0] = [str(ROOT / "reference/upstream"), str(ROOT / "reference/upstream/experiments")]
    from te_harness import swebench as sw, webbench as wb

    if "--native" in sys.argv:
        from native_experiment_task import install
        install()



def data(value):
    if isinstance(value, os.PathLike): return os.fspath(value)
    if isinstance(value, dict): return {key:data(item) for key,item in value.items()}
    if isinstance(value, (list,tuple)): return [data(item) for item in value]
    if isinstance(value, float) and value.is_integer(): return int(value)
    if callable(value): return "callable"
    if value is None or isinstance(value,(str,int,float,bool)): return value
    return str(value)


class TransientFailure(RuntimeError):
    pass

class EvalTimeout(subprocess.TimeoutExpired):
    pass

ShadowRuntimeError = type("RuntimeError", (Exception,), {})
ShadowTimeoutExpired = type("TimeoutExpired", (Exception,), {})

class StableValue:
    def __bool__(self): return False
    def __str__(self): return "fixture value é😀"


for line in sys.stdin:
    request = json.loads(line)
    calls, logs = [], []
    indices = {"clock":0,"segment":0}
    stdout = io.StringIO()
    original_time, original_log, previous_cwd = sw.time.time, sw._log, os.getcwd()
    original_import = sw._import_swebench
    grading_env = {key:os.environ.get(key) for key in ("TE_SWEBENCH_GRADING","DATA","TE_SWEBENCH_REPO")}
    settings_env = {key:os.environ.get(key) for key in ("TE_ENV_ACTIVATION","TE_ALLOW_NETWORK_TOOLS","TE_SWEBENCH_MAX_CPUS")}
    saved_env = os.environ.get("TE_MAX_TURNS")
    interrupt_number=getattr(signal,request["signal"]) if request.get("signal") else None
    interrupt_previous=signal.getsignal(interrupt_number) if interrupt_number is not None else None
    def interrupt(number,frame):
        if number == signal.SIGINT: raise KeyboardInterrupt
        raise SystemExit(number)
    def record(method, args=()):
        calls.append({"method":method,"args":data(args)})
        if interrupt_number is not None and method == request.get("signal_at"):
            os.kill(os.getpid(),interrupt_number)
        if request.get("fail") == method:
            kind = request.get("error_kind","RuntimeError")
            message = request.get("error_message","fixture failure")
            if kind in ("TimeoutExpired","EvalTimeout"):
                output = bytes(request["stdout_bytes"]) if "stdout_bytes" in request else request.get("partial_output")
                klass = EvalTimeout if kind == "EvalTimeout" else subprocess.TimeoutExpired
                raise klass(message,3,output=output)
            if kind == "TransientFailure": raise TransientFailure(message)
            if kind == "ShadowRuntimeError": raise ShadowRuntimeError(message)
            if kind == "ShadowTimeoutExpired": raise ShadowTimeoutExpired(message)
            raise getattr(builtins,kind)(message)
    def clock():
        value = request.get("clock",[100.0,101.25])
        at = indices["clock"]
        indices["clock"] += 1
        return value[min(at,len(value)-1)]
    class Guard:
        segment = 0
        def __bool__(self): return not request.get("falsey_guard",False)
    class Runner(sw.ClaudeSwebenchRunner):
        def __init__(self):
            self.agent, self.model, self.effort, self.cli_version = "claude","claude-opus-4-8","high","fixture-cli"
            self.scaffold = types.SimpleNamespace(name="fixture-scaffold") if request.get("scaffold") else None
        def _container_name(self,*args):
            record("_container_name",args)
            return super()._container_name(*args)
        def _build_guard(self,kwargs):
            record("_build_guard",[kwargs])
            return Guard() if request.get("guard") else None
        def _write_run_config(self,path,task,arm,injected,excluded,turns,kwargs,guard):
            record("_write_run_config",[path,task,arm,injected,excluded,turns,kwargs,guard is not None])
            Path(path,"run_config.json").write_text(json.dumps({"turns":turns,"guard":guard is not None}),encoding="utf-8")
        def _start(self,*args): record("_start",args)
        def _record_env_activation(self,path,name,row):
            record("_record_env_activation",[path,name,row])
            row["env_activation"] = {"mode":"fixture"}
        def _apply_scaffold(self,*args):
            record("_apply_scaffold",args)
            return {"scaffold_setup_s":2.0}, {"fixture":"mcp"}
        def _install_net_guard(self,*args): record("_install_net_guard",args); return True
        def _install_answer_key_hosts(self,*args): record("_install_answer_key_hosts",args); return True
        def _segment(self,name,prompt,stream,excluded,turns,guard,mcp_config=None):
            self._transcript = stream
            record("_segment",[name,prompt,stream.name,excluded,turns,guard is not None,mcp_config])
            n = indices["segment"]
            indices["segment"] += 1
            stream.write(json.dumps({"segment":n,"prompt":prompt},ensure_ascii=False)+"\n")
            stream.flush()
            return {"type":"result","usage":{"input_tokens":10,"cache_read_input_tokens":20,"cache_creation_input_tokens":30,"output_tokens":40},"num_turns":3,"total_cost_usd":0.25}
        def _guard_continuation(self,guard,prompt):
            record("_guard_continuation",[prompt])
            assert not self._transcript.closed, "transcript closed before continuation"
            if guard.segment >= request.get("resumes",1): return None
            guard.segment += 1
            return "resume " + str(guard.segment)
        def _graded_diff(self,*args):
            assert self._transcript.closed, "transcript open during grading"
            record("_graded_diff",args)
            return request.get("patch","fix é😀\n")
        def _grade(self,*args): record("_grade",args); return request.get("resolved",True), {"resolved":request.get("resolved",True)}
        def _net_guard_blocks(self,*args):
            if hasattr(self,"_transcript"): assert self._transcript.closed, "transcript open during cleanup"
            record("_net_guard_blocks",args)
            return 2
        def _stop(self,*args): record("_stop",args)
        def _row_from_segment(self,result):
            record("_row_from_segment",[result])
            return super()._row_from_segment(result)
        def _answer_key_attempts(self,*args): record("_answer_key_attempts",args); return {"attempts":1}
        def _patch_excludes(self): record("_patch_excludes"); return ["fixture.txt"]
        def _guard_row(self,guard): record("_guard_row"); return {"aborted":False,"spiral_nudges":guard.segment}
    class GradeRunner(sw.ClaudeSwebenchRunner):
        def __init__(self):
            self.eval_timeout = request.get("eval_timeout",90)
            def script(iid):
                record("eval_script",[iid])
                return request.get("script","#!/bin/sh\necho é😀")
            def score(*args):
                record("score",args)
                return request.get("score",[True,{"resolved":True}])
            self.grader = types.SimpleNamespace(eval_script=script,score=score)
            self.executions = 0
        def _exec(self,name,script,**kwargs):
            method = "exec_write" if self.executions == 0 else "exec_eval"
            self.executions += 1
            record(method,[name,script,kwargs])
            values = request.get(method,{"returncode":0,"stdout":"EVAL PASSED\né😀\n","stderr":""})
            return types.SimpleNamespace(**values)
        def _eval_exec_failed(self,*args):
            record("_eval_exec_failed",args)
            return super()._eval_exec_failed(*args)
    class GradedTaskRunner(Runner,GradeRunner):
        def __init__(self):
            Runner.__init__(self)
            GradeRunner.__init__(self)
        def _grade(self,*args):
            record("_grade",args)
            return sw.ClaudeSwebenchRunner._grade(self,*args)
    class ConfigRunner(Runner):
        def __init__(self):
            self._record_config = False
            super().__init__()
            for key,value in request.get("metadata",{}).items(): setattr(self,key,value)
            self.agent_bin = Path("/fixture/agent é😀")
            self.routing_base = "https://fixture.invalid"
            self.optimizer_model = "fixture-optimizer"
            if "optimizer_transport" in request: self.optimizer_transport = request["optimizer_transport"]
            self.protected_tools = frozenset(request.get("protected",["Bash","Edit","Write","Read"]))
            self.task_timeout,self.eval_timeout = request.get("task_timeout",1800),request.get("eval_timeout",900)
            self.grader = types.SimpleNamespace(path=Path("fixture-grading.json"))
            self.scaffold_profile = types.SimpleNamespace(patch_excludes=request.get("profile_excludes",["CLAUDE.md"]))
            if self.scaffold is not None:
                class Scaffold:
                    name = "fixture-scaffold"
                    patch_excludes = request.get("scaffold_excludes",["fixture.txt","CLAUDE.md"])
                    def __bool__(self): return not request.get("falsey_scaffold",False)
                self.scaffold = Scaffold()
        def __getattribute__(self,name):
            if name in ("agent","model","effort","cli_version","agent_bin","routing_base","optimizer_model","optimizer_transport","protected_tools","scaffold","scaffold_profile","task_timeout","eval_timeout","grader") and object.__getattribute__(self,"_record_config"):
                record("attr:"+name)
            return super().__getattribute__(name)
        def _write_run_config(self,path,task,arm,injected,excluded,turns,kwargs,guard):
            record("_write_run_config",[path,task,arm,injected,excluded,turns,kwargs,guard is not None])
            self._record_config = True
            try: return sw.ClaudeSwebenchRunner._write_run_config(self,path,task,arm,injected,excluded,turns,kwargs,guard)
            finally: self._record_config = False
        def _patch_excludes(self):
            record("_patch_excludes")
            return sw.ClaudeSwebenchRunner._patch_excludes(self)
    try:
        if interrupt_number is not None: signal.signal(interrupt_number,interrupt)
        sw.time.time, sw._log = clock, logs.append
        if "max_turns_env" in request: os.environ["TE_MAX_TURNS"] = request["max_turns_env"]
        if "cwd" in request: os.chdir(request["cwd"])
        with contextlib.redirect_stdout(stdout):
            kind = request.get("kind","task")
            if kind in ("runner_setting","run_config","configured_task"):
                for key in settings_env:
                    if key in request.get("env",{}): os.environ[key] = request["env"][key]
                    else: os.environ.pop(key,None)
                if kind == "runner_setting":
                    module = wb if request.get("domain") == "webbench" else sw
                    value = getattr(module,request["method"])(**request.get("arguments",{}))
                else:
                    root = Path(request["root"])
                    if not request.get("missing_run_dir"): root.mkdir(parents=True,exist_ok=True)
                    if request.get("config_is_directory"): (root/"run_config.json").mkdir()
                    if request.get("local_cwd"): os.chdir(root)
                    run_dir = os.path.relpath(root) if request.get("run_dir_relative") else str(root)
                    if "run_dir" in request: run_dir = request["run_dir"]
                    task = request.get("task",{"instance_id":"acme__widget-1","repo":"acme/widget","problem_statement":"Fix this task"})
                    kwargs = dict(request.get("kwargs",{}))
                    for key in request.get("object_kwargs",[]): kwargs[key] = StableValue()
                    runner = ConfigRunner()
                    if kind == "configured_task":
                        value = runner.run_task(task,arm="trimech",injected=request.get("injected"),exclude_tools=request.get("excluded"),run_dir_path=run_dir,**kwargs)
                    else:
                        guard = Guard() if request.get("guard") else None
                        if guard is not None:
                            for key,value in request.get("guard_attributes",{}).items(): setattr(guard,key,value)
                        value = runner._write_run_config(run_dir,task,"trimech",request.get("injected"),request.get("excluded"),request.get("max_turns",80),kwargs,guard)
            elif kind in ("official","grading_setting"):
                root = Path(request["root"])
                root.mkdir(parents=True,exist_ok=True)
                for key in grading_env:
                    if key in request.get("env",{}): os.environ[key] = request["env"][key]
                    else: os.environ.pop(key,None)
                if request.get("local_cwd"): os.chdir(root)
                if kind == "grading_setting":
                    value = getattr(sw,request.get("method","default_grading_path"))()
                else:
                    table = root/request.get("table_filename","grading.json")
                    table.parent.mkdir(parents=True,exist_ok=True)
                    if request.get("skip_table"): pass
                    elif request.get("table_is_directory"): table.mkdir()
                    elif "table_bytes" in request: table.write_bytes(bytes(request["table_bytes"]))
                    elif "table_text" in request: table.write_text(request["table_text"],encoding="utf-8")
                    else: table.write_text(json.dumps(request.get("table",{"acme__widget-1":{"FAIL_TO_PASS":"[\"test_a\"]","PASS_TO_PASS":["test_b"]}})),encoding="utf-8")
                    class Spec:
                        def __init__(self,instance): self.instance = instance
                        @property
                        def eval_script(self):
                            record("eval_script_value")
                            return request.get("eval_script_text","echo callback é😀")
                    def make_spec(instance):
                        record("make_test_spec",[instance])
                        return Spec(instance)
                    def report(*,test_spec,prediction,test_log_path,include_tests_status):
                        record("get_eval_report",[test_spec.instance,prediction,test_log_path,include_tests_status])
                        return request.get("report",{"acme__widget-1":{"resolved":True,"extra":"é😀"}})
                    def import_grader():
                        record("import_grader")
                        return make_spec,report
                    sw._import_swebench = import_grader
                    klass = sw._Grader
                    if "instance_override" in request:
                        def instance_override(self,instance_id):
                            record("grader_instance_override",[instance_id])
                            return request["instance_override"]
                        klass = type("FixtureGrader",(klass,),{"instance":instance_override})
                    path = request.get("grading_path",str(table))
                    if request.get("path_object"): path = Path(path)
                    grader = klass.__new__(klass)
                    try:
                        grader.__init__(path)
                        action = request.get("action","instance")
                        iid = request.get("instance_id","acme__widget-1")
                        if action == "instance": value = grader.instance(iid)
                        elif action == "eval_script": value = grader.eval_script(iid)
                        elif action == "score": value = grader.score(iid,request.get("patch","fix é😀\n"),str(root/"eval.log"))
                        else:
                            runner = GradedTaskRunner() if action == "task" else GradeRunner()
                            runner.grader = grader
                            if action == "task":
                                value = runner.run_task({"instance_id":iid,"problem_statement":"Fix this task"},arm="trimech",injected=None,exclude_tools=[],run_dir_path=str(root))
                            else: value = runner._grade("fixture-container",iid,request.get("patch","fix é😀\n"),str(root))
                    finally:
                        calls.append({"grader_state":data({key:getattr(grader,key) for key in ("path","rows") if hasattr(grader,key)}),"path_type":type(grader.path).__name__ if hasattr(grader,"path") else None})
            elif kind == "name":
                runner = types.SimpleNamespace(agent=request.get("agent","claude"),model=request.get("model","claude-opus-4-8"))
                value = sw.ClaudeSwebenchRunner._container_name(runner,**request["arguments"])
            elif kind == "prompt": value = sw.build_prompt(**request["arguments"])
            elif kind == "row":
                klass = {"claude":sw.ClaudeSwebenchRunner,"codex":sw.CodexSwebenchRunner,"copilot":sw.CopilotSwebenchRunner}[request["agent"]]
                value = klass._row_from_segment(None,request["result"])
            elif kind in ("grade","eval_exec_failed"):
                root = Path(request["root"])
                if not request.get("missing_run_dir"): root.mkdir(parents=True,exist_ok=True)
                if request.get("log_is_directory"): (root/"eval.log").mkdir()
                run_dir = os.path.relpath(root) if request.get("run_dir_relative") else str(root)
                if kind == "grade":
                    value = GradeRunner()._grade("fixture-container","acme__widget-1",request.get("patch","fix é😀\n"),run_dir)
                else:
                    value = GradeRunner()._eval_exec_failed("acme__widget-1",str(root/"eval.log"),request.get("out",""),request.get("rc",1),request.get("stderr",""),request.get("phase","running eval.sh"))
            else:
                task = request.get("task",{"instance_id":"acme__widget-1","repo":"acme/widget","problem_statement":"Fix this task"})
                runner = GradedTaskRunner() if request.get("with_grading") else Runner()
                if request.get("wrapper_agent"):
                    agent_class = sw.CopilotSwebenchRunner if request["wrapper_agent"] == "copilot" else sw.CodexSwebenchRunner
                    runner = type("WrappedRunner",(Runner,agent_class),{})()
                    runner.agent,runner.scaffold_name = request["wrapper_agent"],request.get("scaffold_name")
                run_dir = os.path.relpath(request["root"]) if request.get("run_dir_relative") else request["root"]
                row,path = runner.run_task(task,arm="trimech",injected=request.get("injected"),exclude_tools=request.get("excluded"),run_dir_path=run_dir,**request.get("kwargs",{}))
                value = {"row":row,"path":path}
        response = {"value":data(value)}
    except BaseException as error:
        response = {"raised":type(error).__name__,"message":str(error)}
        if isinstance(error,OSError): response["error_info"] = {key:data(getattr(error,key)) for key in ("errno","filename","strerror")}
        if isinstance(error,UnicodeDecodeError): response["error_info"] = {"object":list(error.object),"encoding":error.encoding,"start":error.start,"end":error.end,"reason":error.reason}
        if isinstance(error,json.JSONDecodeError): response["error_info"] = {key:getattr(error,key) for key in ("msg","doc","pos","lineno","colno")}
    finally:
        if interrupt_number is not None:
            response["signal_restored"]=signal.getsignal(interrupt_number) is interrupt
            signal.signal(interrupt_number,interrupt_previous)
        sw.time.time, sw._log = original_time, original_log
        sw._import_swebench = original_import
        for key,value in grading_env.items():
            if value is None: os.environ.pop(key,None)
            else: os.environ[key] = value
        for key,value in settings_env.items():
            if value is None: os.environ.pop(key,None)
            else: os.environ[key] = value
        os.chdir(previous_cwd)
        if saved_env is None: os.environ.pop("TE_MAX_TURNS",None)
        else: os.environ["TE_MAX_TURNS"] = saved_env
    path = Path(request.get("root","/no-task-fixture"))
    response.update(calls=calls,logs=logs,output=stdout.getvalue(),files={p.name:list(p.read_bytes()) for p in sorted(path.glob("*")) if p.is_file()})
    print(json.dumps(response,sort_keys=True,separators=(",",":"),ensure_ascii=False),flush=True)
