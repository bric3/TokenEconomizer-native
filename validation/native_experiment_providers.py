"""Mapping, string-protocol and HTTP bindings for native provider decisions."""
import functools
import builtins
import inspect
import json
import sys

from native_plugin import invoke
from native_experiment_task import checked, data


def install():
    from te_harness import swebench, copilot, codex
    replacements = {}
    def function(module,original,agent,name):
        signature = inspect.signature(original)
        @functools.wraps(original)
        def native(*args,**kwargs):
            bound = signature.bind(*args,**kwargs)
            bound.apply_defaults()
            values = dict(bound.arguments)
            if agent == "codex" and name == "preflight_optimizer":
                from te_harness.swebench import _log
                values["preflight_log"] = _log
            local_env = "env" in values
            if local_env and values["env"] is None and signature.parameters["env"].default is None:
                values["env"] = module.os.environ
            def environment(): return values["env"] if local_env else module.os.environ
            values["output"] = {}
            errors = []
            context,response,document = None,None,""
            result = invoke("experiment.swebench.provider."+agent+"."+name,{})
            while isinstance(result,dict) and "callback" in result:
                method,parameters = result["method"],result["args"]
                try:
                    value = None
                    if method == "provider_env_get": values[parameters[0]] = environment().get(*parameters[1:])
                    elif method == "provider_env_item": values[parameters[0]] = environment()[parameters[1]]
                    elif method == "provider_env_pop": environment().pop(parameters[0],None)
                    elif method == "provider_env_set": environment()[parameters[0]] = values[parameters[1]]
                    elif method == "provider_bool": value = bool(values[parameters[0]])
                    elif method == "provider_is_none": value = values[parameters[0]] is None
                    elif method == "provider_literal": values[parameters[0]] = parameters[1]
                    elif method == "provider_copy": values[parameters[0]] = values[parameters[1]]
                    elif method == "provider_equal": value = bool(values[parameters[0]] == parameters[1])
                    elif method == "provider_string":
                        item = values[parameters[0]]
                        value = item if type(item) is str else None
                    elif method == "provider_method": values[parameters[3]] = getattr(values[parameters[0]],parameters[1])(*parameters[2])
                    elif method == "provider_constant": values[parameters[0]] = getattr(module,parameters[1])
                    elif method == "provider_names": value = list(getattr(module,parameters[0]))
                    elif method == "provider_collect": values["output"][parameters[0]] = values[parameters[1]]
                    elif method == "provider_tuple": values["tuple"] = tuple(values[key] for key in parameters)
                    elif method == "provider_data": value = data(values[parameters[0]])
                    elif method == "provider_log": module._log(*parameters)
                    elif method == "provider_function": values["called"] = getattr(module,parameters[0])(*[values[key] for key in parameters[1]])
                    elif method == "provider_pricing": module.pricing.weights_for({"model":values["model"],"agent":values["agent"]})
                    elif method == "provider_format": value = format(values[parameters[0]],"")
                    elif method == "provider_stringify": value = str(values[parameters[0]])
                    elif method == "provider_repr": value = repr(values[parameters[0]])
                    elif method == "provider_routing_item": values[parameters[0]] = values["routing"][parameters[1]]
                    elif method == "provider_contains": value = values[parameters[0]] in values[parameters[1]]
                    elif method == "provider_items": value = json.dumps(list(values[parameters[0]]),ensure_ascii=True)
                    elif method == "provider_cache_begin":
                        values["cache_key"] = values["base_url"],values["api_key"]
                        value = values["cache_key"] in module._CATALOG
                    elif method == "provider_cache_get": values["catalogue"] = module._CATALOG[values["cache_key"]]
                    elif method == "provider_cache_put": module._CATALOG[values["cache_key"]] = values["catalogue"]
                    elif method == "provider_catalogue_value":
                        values["catalogue"] = None if parameters[0] is None else frozenset(json.loads(item) for item in parameters[0])
                    elif method == "provider_request": values["request"] = module.urllib.request.Request(parameters[0],headers=parameters[1])
                    elif method == "provider_http_enter":
                        context = module.urllib.request.urlopen(values["request"],timeout=values["timeout"])
                        response = context.__enter__()
                    elif method == "provider_http_read":
                        document = response.read().decode()
                        value = {"text":document,"int_max_digits":getattr(sys,"get_int_max_str_digits",lambda:0)()}
                    elif method == "provider_http_exit":
                        info = parameters[0]
                        if info is None: value = bool(context.__exit__(None,None,None))
                        else:
                            error = next((error for error in reversed(errors) if type(error).__name__ == info["kind"] and str(error) == info["message"]),None)
                            if error is None:
                                if info["kind"] == "JSONDecodeError":
                                    details = info["details"]
                                    error = json.JSONDecodeError(details["msg"],details["doc"],details["pos"])
                                else: error = getattr(builtins,info["kind"])(info["message"])
                            value = bool(context.__exit__(type(error),error,error.__traceback__))
                    elif method == "preflight_format": value = format(values[parameters[0]],parameters[1])
                    elif method == "preflight_log":
                        values["preflight_call_log"](*parameters)
                    elif method == "preflight_capture_log": values["preflight_call_log"] = values["preflight_log"] if "preflight_log" in values else module._log
                    elif method == "preflight_invoke":
                        function = getattr(module,parameters[0])
                        keywords = {key:values[item] if kind == "ref" else item for key,(kind,item) in parameters[2].items()}
                        values["probe"],values["effective"] = function(parameters[1],**keywords)
                    else: raise AssertionError("unknown provider callback: "+method)
                    reply = {"value":data(value)}
                except BaseException as error:
                    errors.append(error)
                    reply = {"error":{"kind":type(error).__name__,"message":str(error),"bases":[base.__module__+"."+base.__qualname__ for base in type(error).__mro__]}}
                result = invoke("experiment.swebench.resume",{"job":result["job"],"reply":reply})
            if isinstance(result,dict) and "raised" in result:
                for error in reversed(errors):
                    if type(error).__name__ == result["raised"] and str(error) == result["message"]: raise error
            result = checked(result)
            if isinstance(result,dict) and "provider_ref" in result: return values[result["provider_ref"]]
            if isinstance(result,dict) and "provider_encoded" in result: result = json.loads(result["provider_encoded"])
            return tuple(result) if name == "verify_model" else result
        return native
    for module,agent,names in ((swebench,"claude",("resolve_anthropic_key","agent_credential_env","preflight_optimizer_endpoint")),
                              (copilot,"copilot",("auth_env","byok_active","provider_base","anthropic_optimizer_endpoint","provider_env","provider_secret_names","normalize_model","effort_applies","validate_priceable")),
                              (codex,"codex",("resolve_codex_routing","validate_priceable","fetch_catalog","verify_model","preflight_optimizer"))):
        for name in names:
            original = getattr(module,name)
            replacements[original] = function(module,original,agent,name)
            setattr(module,name,replacements[original])
    webbench = sys.modules.get("te_harness.webbench")
    if webbench is not None:
        for name in ("resolve_anthropic_key","agent_credential_env","preflight_optimizer_endpoint"):
            original = getattr(webbench,name,None)
            if original in replacements: setattr(webbench,name,replacements[original])
    return replacements
