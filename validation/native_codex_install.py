"""Filesystem, process and clock bindings for native Codex installation policy."""
import functools
import inspect

from native_plugin import invoke
from native_experiment_task import checked, data


def install():
    from te_harness import codex as module
    replacements = {}
    def function(original, operation):
        signature = inspect.signature(original)
        @functools.wraps(original)
        def native(*args, **kwargs):
            bound = signature.bind(*args, **kwargs)
            bound.apply_defaults()
            arguments = dict(bound.arguments)
            proc = arguments.pop("proc", None)
            environment = None
            result = invoke("experiment.swebench.install." + operation, data(arguments))
            errors = []
            while isinstance(result,dict) and "callback" in result:
                method, parameters = result["method"], result["args"]
                try:
                    if method == "install_path":
                        owner = module.os if parameters[0] == "listdir" else module.os.path
                        value = getattr(owner,parameters[0])(*parameters[1])
                    elif method == "install_env": value = module.os.environ.get(*parameters)
                    elif method == "install_which": value = module.shutil.which(*parameters)
                    elif method == "install_source_file": value = module.__file__
                    elif method == "install_function": value = getattr(module,parameters[0])(*parameters[1])
                    elif method == "install_read":
                        with open(parameters[0],encoding="utf-8") as fh: value = fh.read()
                    elif method == "install_write":
                        with open(parameters[0],"w",encoding="utf-8") as fh: fh.write(parameters[1])
                        value = None
                    elif method == "install_mkdtemp": value = module.tempfile.mkdtemp(dir=parameters[0])
                    elif method == "install_environment":
                        environment = dict(module.os.environ,**parameters)
                        value = None
                    elif method == "install_rmtree":
                        if parameters[1]: module.shutil.rmtree(parameters[0],ignore_errors=True)
                        else: module.shutil.rmtree(parameters[0])
                        value = None
                    elif method == "install_version":
                        completed = module.subprocess.run([parameters[0],"--version"],capture_output=True,text=True,timeout=60)
                        value = [completed.stdout,completed.stderr]
                    elif method == "install_spawn":
                        proc = module.subprocess.Popen(parameters[0],stdin=module.subprocess.DEVNULL,
                            stdout=module.subprocess.PIPE if parameters[1] else module.subprocess.DEVNULL,
                            stderr=module.subprocess.PIPE if parameters[1] else module.subprocess.DEVNULL,
                            env=environment,start_new_session=True,
                            **({"text":True} if parameters[1] else {}))
                        value = None
                    elif method == "install_communicate": value = proc.communicate(timeout=parameters[0])
                    elif method == "install_poll": value = proc.poll()
                    elif method == "install_killpg":
                        module.os.killpg(proc.pid,module.signal.SIGKILL)
                        value = None
                    elif method == "install_wait":
                        proc.wait(timeout=parameters[0])
                        value = None
                    elif method == "install_cleanup_process": value = module._kill_process_group(proc)
                    elif method == "install_clock": value = module.time.time()
                    elif method == "install_sleep": value = module.time.sleep(*parameters)
                    else: raise AssertionError("unknown installation callback: " + method)
                    reply = {"value":data(value)}
                except BaseException as error:
                    errors.append(error)
                    reply = {"error":{"kind":type(error).__name__,"message":str(error),"bases":[base.__module__ + "." + base.__qualname__ for base in type(error).__mro__]}}
                result = invoke("experiment.swebench.resume",{"job":result["job"],"reply":reply})
            if isinstance(result,dict) and "raised" in result:
                for error in reversed(errors):
                    if type(error).__name__ == result["raised"] and str(error) == result["message"]: raise error
            result = checked(result)
            return tuple(result) if operation == "require_install" else result
        return native
    for name in ("codex_bin","codex_vendor","require_install","cli_version","base_prompt_for",
                 "bundled_system_skills","validate_config","_kill_process_group","_rmtree_quietly"):
        original = getattr(module,name)
        replacements[original] = function(original,name)
        setattr(module,name,replacements[original])
    return replacements
