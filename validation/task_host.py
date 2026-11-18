"""Local container, stream and grader stand-ins for task-lifecycle comparisons."""
import builtins
import contextlib
import io
import json
import os
import subprocess
import sys
import types
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT / "reference/upstream"), str(ROOT / "reference/upstream/experiments")]
from te_harness import swebench as sw

if "--native" in sys.argv:
    from native_experiment_task import install
    install()


def data(value):
    if isinstance(value, os.PathLike): return os.fspath(value)
    if isinstance(value, dict): return {key:data(item) for key,item in value.items()}
    if isinstance(value, (list,tuple)): return [data(item) for item in value]
    if isinstance(value, float) and value.is_integer(): return int(value)
    if callable(value): return "callable"
    return value


class TransientFailure(RuntimeError):
    pass

class EvalTimeout(subprocess.TimeoutExpired):
    pass

ShadowRuntimeError = type("RuntimeError", (Exception,), {})
ShadowTimeoutExpired = type("TimeoutExpired", (Exception,), {})


for line in sys.stdin:
    request = json.loads(line)
    calls, logs = [], []
    indices = {"clock":0,"segment":0}
    stdout = io.StringIO()
    original_time, original_log, previous_cwd = sw.time.time, sw._log, os.getcwd()
    saved_env = os.environ.get("TE_MAX_TURNS")
    def record(method, args=()):
        calls.append({"method":method,"args":data(args)})
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
    try:
        sw.time.time, sw._log = clock, logs.append
        if "max_turns_env" in request: os.environ["TE_MAX_TURNS"] = request["max_turns_env"]
        if "cwd" in request: os.chdir(request["cwd"])
        with contextlib.redirect_stdout(stdout):
            kind = request.get("kind","task")
            if kind == "name":
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
                run_dir = os.path.relpath(request["root"]) if request.get("run_dir_relative") else request["root"]
                row,path = runner.run_task(task,arm="trimech",injected=request.get("injected"),exclude_tools=request.get("excluded"),run_dir_path=run_dir,**request.get("kwargs",{}))
                value = {"row":row,"path":path}
        response = {"value":data(value)}
    except BaseException as error:
        response = {"raised":type(error).__name__,"message":str(error)}
    finally:
        sw.time.time, sw._log = original_time, original_log
        os.chdir(previous_cwd)
        if saved_env is None: os.environ.pop("TE_MAX_TURNS",None)
        else: os.environ["TE_MAX_TURNS"] = saved_env
    path = Path(request.get("root","/no-task-fixture"))
    response.update(calls=calls,logs=logs,output=stdout.getvalue(),files={p.name:p.read_text(encoding="utf-8") for p in sorted(path.glob("*")) if p.is_file()})
    print(json.dumps(response,sort_keys=True,separators=(",",":"),ensure_ascii=False),flush=True)
