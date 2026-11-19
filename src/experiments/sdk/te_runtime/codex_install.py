"""Filesystem, process and clock bindings for native Codex installation policy."""
import os
import shutil
import signal
import subprocess
import tempfile
import time
from types import SimpleNamespace

from .transport import checked, data


def bind(bridge,owner=None):
    if owner is None:
        owner=SimpleNamespace(os=os,shutil=shutil,signal=signal,subprocess=subprocess,
            tempfile=tempfile,time=time,__file__=__file__)

    def call(operation,arguments,proc=None):
        with bridge.frames(): return drive(operation,arguments,proc)

    def drive(operation,arguments,proc):
        environment=None
        errors=[]
        result=bridge.request("experiment.swebench.install."+operation,data(arguments))
        while isinstance(result,dict) and "callback" in result:
            method,p=result["method"],result["args"]
            try:
                with bridge.callback_scope():
                    bridge.deliver_signals()
                    value=None
                    if method == "install_path":
                        target=owner.os if p[0] == "listdir" else owner.os.path
                        value=getattr(target,p[0])(*p[1])
                    elif method == "install_env": value=owner.os.environ.get(*p)
                    elif method == "install_which": value=owner.shutil.which(*p)
                    elif method == "install_function": value=getattr(owner,p[0])(*p[1])
                    elif method == "install_read":
                        with open(p[0],encoding="utf-8") as handle: value=handle.read()
                    elif method == "install_write":
                        with open(p[0],"w",encoding="utf-8") as handle: handle.write(p[1])
                    elif method == "install_mkdtemp": value=owner.tempfile.mkdtemp(dir=p[0])
                    elif method == "install_environment": environment=dict(owner.os.environ,**p)
                    elif method == "install_rmtree":
                        if p[1]: owner.shutil.rmtree(p[0],ignore_errors=True)
                        else: owner.shutil.rmtree(p[0])
                    elif method == "install_version":
                        completed=owner.subprocess.run([p[0],"--version"],capture_output=True,text=True,timeout=60)
                        value=[completed.stdout,completed.stderr]
                    elif method == "install_spawn":
                        proc=owner.subprocess.Popen(p[0],stdin=owner.subprocess.DEVNULL,
                            stdout=owner.subprocess.PIPE if p[1] else owner.subprocess.DEVNULL,
                            stderr=owner.subprocess.PIPE if p[1] else owner.subprocess.DEVNULL,
                            env=environment,start_new_session=True,**({"text":True} if p[1] else {}))
                    elif method == "install_communicate": value=proc.communicate(timeout=p[0])
                    elif method == "install_poll": value=proc.poll()
                    elif method == "install_killpg": owner.os.killpg(proc.pid,owner.signal.SIGKILL)
                    elif method == "install_wait": proc.wait(timeout=p[0])
                    elif method == "install_cleanup_process": value=owner._kill_process_group(proc)
                    elif method == "install_clock": value=owner.time.time()
                    elif method == "install_sleep": value=owner.time.sleep(*p)
                    else: raise ValueError("unknown installation callback: "+method)
                    reply={"value":data(value)}
            except BaseException as error:
                errors.append(error)
                reply={"error":{"kind":type(error).__name__,"message":str(error),
                    "details":{"provider_error_index":len(errors)-1},
                    "bases":[base.__module__+"."+base.__qualname__ for base in type(error).__mro__]}}
            result=bridge.request("experiment.swebench.resume",{"job":result["job"],"reply":reply})
        if isinstance(result,dict) and "raised" in result:
            index=(result.get("details") or {}).get("provider_error_index")
            if index is not None: raise errors[index]
        result=checked(result)
        bridge.deliver_signals()
        return result

    def codex_bin(): return call("codex_bin",{})
    def codex_vendor(binary=None): return call("codex_vendor",dict(binary=binary))
    def require_install(): return tuple(call("require_install",{}))
    def cli_version(binary): return call("cli_version",dict(binary=binary))
    def base_prompt_for(cli_version): return call("base_prompt_for",dict(cli_version=cli_version))
    def bundled_system_skills(binary=None,home=None,timeout=45):
        return call("bundled_system_skills",dict(binary=binary,home=home,timeout=timeout))
    def validate_config(text,*,binary=None,home=None,timeout=90):
        return call("validate_config",dict(text=text,binary=binary,home=home,timeout=timeout))
    def _kill_process_group(proc): return call("_kill_process_group",{},proc)
    def _rmtree_quietly(path): return call("_rmtree_quietly",dict(path=path))

    vars(owner).update(codex_bin=codex_bin,codex_vendor=codex_vendor,require_install=require_install,
        cli_version=cli_version,base_prompt_for=base_prompt_for,bundled_system_skills=bundled_system_skills,
        validate_config=validate_config,_kill_process_group=_kill_process_group,_rmtree_quietly=_rmtree_quietly)
    return owner
