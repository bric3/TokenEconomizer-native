"""Process stand-ins and captured source installers for guard integration checks."""
import base64
import json
import os
from pathlib import Path
import subprocess
import sys
import types

ROOT = Path(__file__).resolve().parents[1]
if "--sdk" in sys.argv:
    import standalone_harness as task_binding
    sw=task_binding.sw
else:
    sys.path[:0] = [str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
    from te_harness import swebench as sw
    if "--native" in sys.argv:
        import native_experiment_task as task_binding
        from native_experiment_task import install
        install()
        from native_experiment_net_guard import install
        install(sw.net_guard)

for line in sys.stdin:
    request = json.loads(line)
    calls,logs = [],[]
    mode = request.get("mode","hosts")
    agent = request.get("agent","claude")
    runner = object.__new__({"claude":sw.ClaudeSwebenchRunner,"codex":sw.CodexSwebenchRunner,"copilot":sw.CopilotSwebenchRunner}[agent])
    runner.scaffold_profile = types.SimpleNamespace(home=request.get("home","/fixture/home"),kind=agent)
    saved_env = os.environ.get("TE_ALLOW_NETWORK_TOOLS")
    saved_log,saved_block,saved_run = sw._log,sw.network_tool_block,sw._run
    saved_root = task_binding.ROOT if "--native" in sys.argv else None
    saved_digits = sys.get_int_max_str_digits()
    def record(method,args):
        calls.append([method,args])
        if request.get("fail") == method:
            raise getattr(__import__("builtins"),request.get("error","RuntimeError"))("fixture failure")
    def execute(name,script,**kwargs):
        index = sum(call[0] == "exec" for call in calls)
        record("exec",[name,script,kwargs])
        if request.get("fail_at") == index:
            raise getattr(__import__("builtins"),request.get("error","RuntimeError"))(request.get("message","fixture failure"))
        if mode == "program": return subprocess.CompletedProcess(script,0,"armed\n","")
        if request.get("live"):
            return subprocess.run(["docker","exec",name,"bash","-lc",script],capture_output=True,text=True,**kwargs)
        default = "1\n" if mode == "hosts" and index == 0 else "127.0.0.1 github.com\n" if mode == "hosts" else "rc=2\n" if mode == "verify" else "2\n" if mode == "blocks" else "patch é😀\n"
        if mode == "install": default=["aarch64\n","ready\n","armed\n","rc=2\n"][min(index,3)]
        if mode in ("write","register"): default="armed\n"
        values = request.get("responses",[])
        value = values[index] if index < len(values) else {}
        return subprocess.CompletedProcess(script,value.get("returncode",0),value.get("stdout",default),value.get("stderr",""))
    def run(argv,**kwargs):
        record("run",[argv,kwargs])
        if request.get("live"): return subprocess.run(argv,capture_output=True,text=True,**kwargs)
        value=request.get("copy_response",{})
        return subprocess.CompletedProcess(argv,value.get("returncode",0),value.get("stdout",""),value.get("stderr",""))
    def excludes():
        record("excludes",[])
        value = request.get("excludes",[])
        return iter(value) if request.get("iterator") else value
    def network():
        record("network",[])
        return saved_block()
    runner._exec,runner._patch_excludes = execute,excludes
    saved_attempts=sw.answer_key_attempts
    if "program" in request:
        def program(entry): record("program",[entry]);return request["program"]
        runner._net_guard_merge_program=program
    if "writer_result" in request:
        def writer(*args): record("writer",args);return request["writer_result"]
        runner._write_hook_entry=writer
    if "attempts_result" in request:
        def attempts(*args): record("attempts",args);return request["attempts_result"]
        sw.answer_key_attempts=attempts
    try:
        sw._log,sw.network_tool_block = logs.append,network
        if mode == "install":
            sw._run=run
            if "artifact_root" in request and "--native" in sys.argv: task_binding.ROOT=Path(request["artifact_root"])
        if "allow" in request: os.environ["TE_ALLOW_NETWORK_TOOLS"] = request["allow"]
        else: os.environ.pop("TE_ALLOW_NETWORK_TOOLS",None)
        if "int_max_digits" in request: sys.set_int_max_str_digits(request["int_max_digits"])
        if mode == "program":
            if "entry" in request:
                program=sw.ClaudeSwebenchRunner._net_guard_merge_program(request["entry"])
                print(base64.b64encode(program.encode()).decode(),flush=True)
                continue
            runner._register_net_guard("fixture",request["python"])
            program = calls[0][1][1].split("<<'TE_EOF'\n",1)[1].split("\nTE_EOF\n",1)[0]
            print(base64.b64encode(program.encode()).decode(),flush=True)
            continue
        name = request.get("name","fixture")
        if mode == "paths": value=[runner.NET_GUARD_SCRIPT,runner.NET_GUARD_LOG]
        elif mode == "attempts":
            value=runner._answer_key_attempts(request.get("run_dir","fixture"))
            assert value is request["attempts_result"],"attempt result lost identity"
        elif mode == "write": value=runner._write_hook_entry(name,request.get("python","/usr/bin/python3"),request.get("path","/fixture/settings.json"))
        elif mode == "register": value=runner._register_net_guard(name,request.get("python","/usr/bin/python3"))
        elif mode == "hosts": value = runner._install_answer_key_hosts(name)
        elif mode == "install": value = runner._install_net_guard(name)
        elif mode == "verify": value = runner._verify_net_guard(name,request.get("python","/usr/bin/python3"))
        elif mode == "blocks": value = runner._net_guard_blocks(name)
        else: value = runner._graded_diff(name)
        response = {"value":value}
        if mode in ("install","register"): response["hook_trust"]=getattr(runner,"_guard_python","")
    except BaseException as error:
        response = {"raised":type(error).__name__,"message":str(error)}
    finally:
        sw._log,sw.network_tool_block,sw._run = saved_log,saved_block,saved_run
        sw.answer_key_attempts=saved_attempts
        if "--native" in sys.argv: task_binding.ROOT=saved_root
        sys.set_int_max_str_digits(saved_digits)
        if saved_env is None: os.environ.pop("TE_ALLOW_NETWORK_TOOLS",None)
        else: os.environ["TE_ALLOW_NETWORK_TOOLS"] = saved_env
    response.update(calls=calls,logs=logs)
    sys.set_int_max_str_digits(0)
    try: print(json.dumps(response,sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
    finally: sys.set_int_max_str_digits(saved_digits)
