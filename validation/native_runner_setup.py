"""Opaque SDK references for native SWE-bench constructor coordination."""
import functools
import inspect
import sys

from native_plugin import invoke
from native_experiment_task import checked, data


def install():
    from te_harness import swebench as module
    replacements = {}
    def constructor(original,agent):
        signature = inspect.signature(original)
        @functools.wraps(original)
        def native(self,*args,**kwargs):
            bound = signature.bind(self,*args,**kwargs)
            bound.apply_defaults()
            arguments = dict(bound.arguments)
            returned = None
            locals_ = {}
            errors = []
            def member(name):
                parts = name.split(".")
                obj = self if parts[0] == "self" else getattr(module,parts[0])
                for part in parts[1:]: obj = getattr(obj,part)
                return obj
            def resolve(reference):
                kind,*parameters = reference
                if kind == "field": return getattr(self,parameters[0])
                if kind == "arg": return arguments[parameters[0]]
                if kind == "module": return member(parameters[0])
                if kind == "result": return returned
                if kind == "value": return parameters[0]
                if kind == "integer": return int(parameters[0])
                if kind == "empty_tuple": return ()
                if kind == "tuple": return tuple(returned)
                if kind == "mapping": return {key:resolve(item) for key,item in parameters[0].items()}
                if kind == "nested_field": return getattr(getattr(self,parameters[0]),parameters[1])
                if kind == "item": return getattr(self,parameters[0])[parameters[1]]
                if kind == "local": return locals_[parameters[0]]
                raise AssertionError("unknown constructor reference: "+kind)
            result = invoke("experiment.swebench.setup."+agent,{})
            while isinstance(result,dict) and "callback" in result:
                method,parameters = result["method"],result["args"]
                try:
                    if method == "setup_read":
                        value = resolve(parameters)
                        if isinstance(value,(set,frozenset)): value = list(value)
                    elif method == "setup_bool": value = bool(resolve(parameters))
                    elif method == "setup_present": value = resolve(parameters) is not None
                    elif method == "setup_len": value = len(resolve(parameters))
                    elif method == "setup_assign":
                        setattr(self,parameters[0],resolve(parameters[1]))
                        value = None
                    elif method == "setup_capture":
                        for name,reference in parameters: locals_[name] = resolve(reference)
                        value = None
                    elif method == "setup_call":
                        function = member(parameters[0]) if isinstance(parameters[0],str) else resolve(parameters[0])
                        positional = [resolve(item) for item in parameters[1]]
                        keywords = {key:resolve(item) for key,item in parameters[2].items()}
                        returned = function(*positional,**keywords)
                        value = None
                    elif method == "setup_pair":
                        left,right = returned
                        for (kind,name),item in zip(parameters,(left,right)):
                            if kind == "field": setattr(self,name,item)
                            else: locals_[name] = item
                        value = None
                    elif method == "setup_env": value = module.os.environ.get(*parameters)
                    elif method == "setup_integer_limit": value = getattr(sys,"get_int_max_str_digits",lambda:0)()
                    else: raise AssertionError("unknown constructor callback: "+method)
                    reply = {"value":data(value)}
                except BaseException as error:
                    errors.append(error)
                    reply = {"error":{"kind":type(error).__name__,"message":str(error),"bases":[base.__module__ + "." + base.__qualname__ for base in type(error).__mro__]}}
                result = invoke("experiment.swebench.resume",{"job":result["job"],"reply":reply})
            if isinstance(result,dict) and "raised" in result:
                for error in reversed(errors):
                    if type(error).__name__ == result["raised"] and str(error) == result["message"]: raise error
            return checked(result)
        return native
    for name,agent in (("ClaudeSwebenchRunner","claude"),("CopilotSwebenchRunner","copilot"),("CodexSwebenchRunner","codex")):
        cls = getattr(module,name)
        original = cls.__init__
        replacements[original] = constructor(original,agent)
        cls.__init__ = replacements[original]
    return replacements
