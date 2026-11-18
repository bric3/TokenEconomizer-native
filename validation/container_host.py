"""Local subprocess stand-ins for source/native container comparisons."""
import contextlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import types

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
from te_harness import swebench as sw

if "--native" in sys.argv:
    from native_experiment_task import install
    install()

def data(value):
    if isinstance(value,os.PathLike): return os.fspath(value)
    if isinstance(value,dict): return {key:data(item) for key,item in value.items()}
    if isinstance(value,(list,tuple,set)): return [data(item) for item in value]
    return value

KEYS = ("TE_ENV_ACTIVATION","TE_DOCKER_START_TIMEOUT","TE_DOCKER_STOP_TIMEOUT","TE_SWEBENCH_MAX_CPUS","COPILOT_HOST_CACHE","COPILOT_HOME")
NAME = "te-cell-abc"

for line in sys.stdin:
    request = json.loads(line)
    calls,logs,processes = [],[],[]
    row = {"keep":"row"}
    saved = {key:os.environ.get(key) for key in KEYS}
    original_run, original_log, original_subprocess, original_cwd = sw._run,sw._log,subprocess.run,os.getcwd()
    original_live = set(sw._LIVE_CONTAINERS)
    stdout = io.StringIO()
    def record(method,args=()):
        calls.append({"method":method,"args":data(args)})
        if request.get("fail") == method:
            kind = request.get("error_kind","RuntimeError")
            if kind == "TimeoutExpired": raise subprocess.TimeoutExpired("fixture command",3,output="partial")
            if kind == "ShadowRuntimeError": raise type("RuntimeError",(Exception,),{})("fixture failure")
            raise getattr(__import__("builtins"),kind)("fixture failure")
    def backend(cmd,timeout=120,check=False):
        index = len(processes)
        record("run",[cmd,timeout,check])
        if request.get("fail_at") == index:
            kind = request.get("error_kind","RuntimeError")
            if kind == "TimeoutExpired": raise subprocess.TimeoutExpired(cmd,timeout,output="partial")
            if kind == "ShadowRuntimeError": raise type("RuntimeError",(Exception,),{})("fixture failure")
            raise getattr(__import__("builtins"),kind)("fixture failure")
        command = cmd if isinstance(cmd,list) else [cmd]
        stage = "exec"
        if command[:2] == ["docker","ps"]: stage = "ps"
        elif command[:2] == ["docker","rm"]: stage = "remove"
        elif command[:2] == ["docker","run"]: stage = "start"
        elif "mkdir" in command: stage = "mkdir"
        elif command and "ENV=%s" in str(command[-1]): stage = "discovery"
        elif command and str(command[-1]).startswith("command -v python;"): stage = "check"
        elif command and str(command[-1]).startswith("mkdir -p /home/agent"): stage = "seed"
        default = {"ps":"te-cell-old "+NAME,"discovery":"PATH=/usr/bin:/bin\nENV=/opt/miniconda3/envs/testbed/bin\n","check":"/opt/miniconda3/envs/testbed/bin/python\n/opt/miniconda3/envs/testbed\n"}.get(stage,"fixture output\n")
        response = request.get("responses",{}).get(stage,{})
        result = subprocess.CompletedProcess(cmd,response.get("returncode",0),response.get("stdout",default),response.get("stderr",""))
        processes.append(result)
        return result
    def sdk_backend(cmd,**kwargs):
        record("subprocess",[cmd,kwargs])
        response = request.get("process",{})
        result = subprocess.CompletedProcess(cmd,response.get("returncode",0),response.get("stdout","output\n"),response.get("stderr",""))
        processes.append(result)
        return result
    class Scaffold:
        def mounts(self,profile):
            record("mounts",[profile.home])
            return request.get("mounts",["/fixture/source:/fixture/target:ro"])
        def __bool__(self): return not request.get("falsey_scaffold",False)
    try:
        for key in KEYS:
            if key in request.get("env",{}): os.environ[key] = request["env"][key]
            else: os.environ.pop(key,None)
        sw._LIVE_CONTAINERS.clear()
        sw._LIVE_CONTAINERS.update(request.get("live",[] if request.get("mode") in ("start","flow") else [NAME]))
        sw._log,sw._run = logs.append,backend
        root = Path(request["root"])
        root.mkdir(parents=True,exist_ok=True)
        if request.get("local_cwd"): os.chdir(root)
        agent = request.get("agent","claude")
        runner = object.__new__({"claude":sw.ClaudeSwebenchRunner,"copilot":sw.CopilotSwebenchRunner,"codex":sw.CodexSwebenchRunner}[agent])
        runner.keep = request.get("keep",False)
        runner.claude,runner.copilot,runner.codex_vendor = "/fixture/claude","/fixture/copilot","/fixture/codex-vendor"
        runner.scaffold = Scaffold() if request.get("scaffold") else None
        runner.scaffold_profile = types.SimpleNamespace(home="/fixture/agent-home")
        if "state" in request: runner._env_activation = request["state"]
        runner._write_config = lambda name: record("_write_config",[name])
        mode = request.get("mode","activate")
        if mode in ("record","flow"):
            path = root/"run_config.json"
            if request.get("config_is_directory"): path.mkdir()
            elif not request.get("missing_config"):
                path.write_text(json.dumps(request.get("config",{"arm":"fixture"})),encoding="utf-8")
        path_arg = os.path.relpath(root) if request.get("relative_path") else str(root)
        with contextlib.redirect_stdout(stdout):
            if mode == "activate":
                value = runner._activate_project_env(NAME)
                assert value is runner._env_activation[NAME], "activation lost stored identity"
            elif mode == "info":
                value = runner._env_activation_info(NAME)
                if request.get("state",{}).get(NAME): assert value is runner._env_activation[NAME], "info lost stored identity"
            elif mode == "args": value = runner._env_activation_args(NAME)
            elif mode == "record": value = runner._record_env_activation(path_arg,NAME,row)
            elif mode == "start": value = runner._start(NAME,"acme__widget-1")
            elif mode == "stop": value = runner._stop(NAME)
            elif mode == "reap": value = runner._reap_exited(NAME)
            elif mode == "exec":
                value = runner._exec(NAME,request.get("script","echo é😀"),timeout=request.get("timeout",300),workdir=request.get("workdir","/testbed"))
                assert value is processes[-1], "exec lost process identity"
                value = {key:getattr(value,key) for key in ("args","returncode","stdout","stderr")}
            elif mode == "raw_run":
                sw._run = original_run
                subprocess.run = sdk_backend
                command = request.get("cmd",["fake","a","b","c","d"])
                value = sw._run(command,timeout=request.get("timeout",120),check=request.get("check",False))
                assert value is processes[-1], "run lost process identity"
                assert value.args is command, "run copied command identity"
                value = {key:getattr(value,key) for key in ("args","returncode","stdout","stderr")}
            elif mode == "copilot_cache": value = sw.copilot.copilot_cache()
            elif mode == "copilot_config": value = sw.copilot.copilot_config()
            elif mode == "copilot_mounts": value = sw.copilot.container_mounts("/fixture/copilot")
            elif mode == "codex_mounts": value = sw.codex.container_mounts("/fixture/vendor")
            elif mode == "seed": value = sw.copilot.container_seed_setup()
            else:
                runner._start(NAME,"acme__widget-1")
                runner._record_env_activation(path_arg,NAME,row)
                args = runner._env_activation_args(NAME)
                runner._exec(NAME,"echo hello")
                runner._stop(NAME)
                value = args
        response = {"value":data(value)}
    except BaseException as error:
        response = {"raised":type(error).__name__,"message":str(error)}
        if isinstance(error,OSError): response["error_info"] = {key:getattr(error,key) for key in ("errno","filename","strerror")}
    finally:
        response.update(calls=calls,logs=logs,row=row,live=sorted(sw._LIVE_CONTAINERS),state=data(runner.__dict__.get("_env_activation")) if "runner" in locals() else None,output=stdout.getvalue())
        response["files"] = {path.name:list(path.read_bytes()) for path in sorted(root.glob("*")) if path.is_file()}
        sw._run,sw._log,subprocess.run = original_run,original_log,original_subprocess
        sw._LIVE_CONTAINERS.clear();sw._LIVE_CONTAINERS.update(original_live)
        os.chdir(original_cwd)
        for key,value in saved.items():
            if value is None: os.environ.pop(key,None)
            else: os.environ[key] = value
    print(json.dumps(response,sort_keys=True,separators=(",",":"),ensure_ascii=False),flush=True)
