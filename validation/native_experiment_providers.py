"""Mapping, string-protocol and HTTP bindings for native provider decisions."""
import functools
import builtins
import inspect
import operator
import re
import json
import sys
from decimal import Decimal

from native_plugin import invoke
from native_experiment_task import checked, data


def install(runner_module=None):
    from te_harness import swebench, copilot, codex
    replacements = {}
    def function(module,original,agent,name):
        signature = inspect.signature(original)
        @functools.wraps(original)
        def native(*args,**kwargs):
            try: bound = signature.bind(*args,**kwargs)
            except TypeError: bound = None
            if bound is None:
                # Python reports argument errors before the original body can run.
                return original(*args,**kwargs)
            bound.apply_defaults()
            values = dict(bound.arguments)
            deferred_errors = name in ("_log_optimizer_call","_log") or name.startswith("runner_") or agent == "webbench"
            if agent == "codex" and name == "optimizer_call":
                from te_harness.swebench import _log, _log_optimizer_call
                values["optimizer_log"] = _log
                values["optimizer_record"] = _log_optimizer_call
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
            # Native arithmetic can produce more digits than the HTTP decoder permits.
            def decode(encoded): return json.loads(encoded,parse_int=lambda number:int(Decimal(number)))
            def error_object(info):
                index = (info.get("details") or {}).get("provider_error_index")
                return errors[index] if index is not None else getattr(builtins,info["kind"])(info["message"])
            def special(instance,name):
                method = inspect.getattr_static(type(instance),name)
                descriptor = getattr(type(method),"__get__",None)
                return descriptor(method,instance,type(instance)) if descriptor is not None else method
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
                        values["optimizer_suppressed"] = info is not None and value
                    elif method == "optimizer_json": value = json.dumps(values[parameters[0]],ensure_ascii=True)
                    elif method == "optimizer_string":
                        item = values[parameters[0]]
                        # Code point arrays preserve adjacent surrogates as distinct characters.
                        value = list(map(ord,item)) if type(item) is str else None
                    elif method == "optimizer_format": value = json.dumps(format(values[parameters[0]],""),ensure_ascii=True)
                    elif method == "optimizer_capture_request": values["request_function"] = module.urllib.request.Request
                    elif method == "optimizer_request":
                        headers = {key:values[item] if kind == "ref" else decode(item) if kind == "encoded" else item for key,kind,item in parameters[2]}
                        values["request"] = values["request_function"](decode(parameters[0]),data=parameters[1].encode(),method="POST",headers=headers)
                    elif method == "optimizer_capture_log": values["active_log"] = values["optimizer_log"] if "optimizer_log" in values else module._log
                    elif method == "optimizer_log": values["active_log"](decode(parameters[0]))
                    elif method == "optimizer_sleep": module.time.sleep(*parameters)
                    elif method == "optimizer_error_code":
                        values["error_code"] = values["http_error"].code
                        value = values["error_code"]
                    elif method == "optimizer_error_body": value = list(values["http_error"].read()[:parameters[0]])
                    elif method == "optimizer_suppressed": value = values.get("optimizer_suppressed",False)
                    elif method == "optimizer_integer_limit": value = getattr(sys,"get_int_max_str_digits",lambda:0)()
                    elif method == "optimizer_set_usage": values["usage"] = decode(parameters[0])
                    elif method == "optimizer_usage_from_response": values["usage"] = values["data"].get("usage")
                    elif method == "optimizer_usage_field": value = json.dumps(values["usage"].get(parameters[0]),ensure_ascii=True)
                    elif method == "optimizer_capture_usage": values[parameters[0]] = values["usage"].get(parameters[0])
                    elif method == "optimizer_response_text":
                        function = module._responses_text
                        values["data"] = decode(parameters[0])
                        values["text"] = function(values["data"])
                        value = {"text":json.dumps(values["text"],ensure_ascii=True),"data":json.dumps(values["data"],ensure_ascii=True)}
                    elif method == "optimizer_record":
                        function = values["optimizer_record"] if "optimizer_record" in values else module._log_optimizer_call
                        values["text"] = decode(parameters[0])
                        usage = values["usage"] if parameters[1] is None else decode(parameters[1])
                        function(values["mid"],values["prompt"],values["text"],usage,stop_reason=decode(parameters[2]))
                    elif method == "optimizer_pricing":
                        record = {"model":values["mid"]}
                        record.update({key:values[item] if kind == "ref" else int(Decimal(item)) if kind == "integer" else item for key,(kind,item) in parameters.items()})
                        values["effective"] = module.pricing.effective_tokens(record)
                    elif method == "optimizer_finish": values["optimizer_result"] = decode(parameters[0]),values["effective"]
                    elif method == "cli_binary": values["binary"] = module.copilot_bin()
                    elif method == "cli_temp": values[parameters[0]] = module.tempfile.mkdtemp(prefix=parameters[1])
                    elif method == "cli_tools":
                        values[parameters[0]] = list(getattr(module,parameters[0]))
                        value = len(values[parameters[0]])
                    elif method == "cli_stringify": values[parameters[1]] = str(values[parameters[0]])
                    elif method == "cli_capture_run":
                        values["run"] = module.subprocess.run
                        values["run_environ"] = module.os.environ
                    elif method == "cli_run_env": values["run_env"] = module.run_env()
                    elif method == "cli_run":
                        argv = [values[item] if kind == "ref" else values[item][rest[0]] if kind == "tool" else item for kind,item,*rest in parameters[0]]
                        values["stdout"] = values["run"](argv,capture_output=True,text=True,timeout=values["timeout"],env=dict(values["run_environ"],**values["run_env"])).stdout
                    elif method == "cli_remove": module.shutil.rmtree(values[parameters[0]],ignore_errors=True)
                    elif method == "cli_lines": value = json.dumps(values["stdout"].splitlines(),ensure_ascii=True)
                    elif method == "cli_set_text": values["text"] = decode(parameters[0])
                    elif method == "cli_capture_sum": values["sum"] = module.sum_openai_usage
                    elif method == "cli_read_log": values["log_text"] = module.read_log_dir(values["logdir"])
                    elif method == "cli_sum": values["usage"] = values["sum"](values["log_text"])
                    elif method == "cli_capture_record":
                        from te_harness.swebench import _log_optimizer_call
                        values["record"] = _log_optimizer_call
                    elif method == "cli_record": values["record"](values["model"],values["prompt"],values["text"],values["observed_usage"])
                    elif method == "cli_pricing": values["effective"] = module.pricing.effective_tokens(values["usage"])
                    elif method == "cli_finish": values["optimizer_result"] = values["text"],values["effective"]
                    elif method == "host_path": values[parameters[1]] = getattr(module.os.path,parameters[0])(values[parameters[2]])
                    elif method == "host_function": values[parameters[1]] = getattr(module,parameters[0])()
                    elif method == "host_access": values["accessible"] = module.os.access(values["home"],module.os.X_OK)
                    elif method == "host_which": values["binary"] = module.shutil.which("copilot")
                    elif method == "host_version": values["completed"] = module.subprocess.run([values["binary"],"--version"],capture_output=True,text=True,timeout=60)
                    elif method == "host_stream": values["stream"] = getattr(values["completed"],parameters[0])
                    elif method == "host_first_version": values["version"] = values["lines"][0][:120]
                    elif method == "host_missing": value = json.dumps(", ".join(values[key] for key in parameters[0]),ensure_ascii=True)
                    elif method == "host_update": values[parameters[0]].update(values[parameters[1]])
                    elif method == "context_log_path": values["path"] = module.os.path.join(values["root"],"logs","optimizer_calls.jsonl")
                    elif method == "context_capture_path": values[parameters[1]] = getattr(module.os.path,parameters[0])
                    elif method == "context_apply_path": values[parameters[1]] = values[parameters[0]](*[values[key] for key in parameters[2]])
                    elif method == "context_clear": module._OPT_CTX.clear()
                    elif method == "context_capture_update": values["update"] = module._OPT_CTX.update
                    elif method == "context_fields":
                        values["fields_items"] = list(values["fields"].items())
                        value = [item is None for key,item in values["fields_items"]]
                    elif method == "context_update": values["update"](dict(values["fields_items"][index] for index in parameters[0]))
                    elif method == "context_slice_head": values["head"] = values["prompt"][:600]
                    elif method == "context_head": values["head"] = "".join(map(chr,parameters[0]))
                    elif method == "context_kinds": values["kinds"] = iter(module._OPT_KINDS)
                    elif method == "context_next_kind":
                        try: item = next(values["kinds"])
                        except StopIteration: value = [False,None]
                        else:
                            values["kind"],values["signature"] = item
                            value = [True,list(map(ord,values["signature"])) if type(values["signature"]) is str else None]
                    elif method == "logger_path": values["path"] = module.optimizer_log_path()
                    elif method == "logger_capture_makedirs": values["makedirs"] = module.os.makedirs
                    elif method == "logger_makedirs": values["makedirs"](values["parent"],exist_ok=True)
                    elif method == "logger_open":
                        values["log_context"] = getattr(module,"open",builtins.open)(values["path"],"a",encoding="utf-8")
                        # The source runtime resolves __exit__ before __enter__.
                        values["log_exit"] = special(values["log_context"],"__exit__")
                        enter = special(values["log_context"],"__enter__")
                        values["handle"] = enter()
                    elif method == "logger_capture_write": values["write"] = values["handle"].write
                    elif method == "logger_timestamp": values["timestamp"] = module.time.strftime(parameters[0],module.time.gmtime())
                    elif method == "logger_context": values["row"] = {"ts":values["timestamp"],**module._OPT_CTX}
                    elif method == "logger_kind": values["kind"] = module.optimizer_call_kind(values["prompt"])
                    elif method == "logger_field": values["row"][parameters[0]] = values[parameters[1]]
                    elif method == "logger_equal": values[parameters[2]] = values[parameters[0]] == parameters[1]
                    elif method == "logger_slice": values[parameters[3]] = values[parameters[0]][parameters[1]:parameters[2]]
                    elif method == "logger_literal": values[parameters[0]] = "".join(map(chr,parameters[1]))
                    elif method == "logger_length": values[parameters[1]] = len(values[parameters[0]])
                    elif method == "logger_encode": value = json.dumps(values["row"])
                    elif method == "logger_write": values["write"](parameters[0])
                    elif method == "logger_close":
                        if parameters[0] is None: values["log_exit"](None,None,None)
                        else:
                            error = error_object(parameters[0])
                            if not values["log_exit"](type(error),error,error.__traceback__): raise error
                    elif method == "logger_error_format": value = list(map(ord,format(error_object(parameters[0]),"")))
                    elif method == "logger_format": value = list(map(ord,format(values[parameters[0]],"")))
                    elif method == "logger_report": values["active_log"]("".join(map(chr,parameters[0])))
                    elif method == "logger_capture_print": values["print"] = getattr(module,"print",builtins.print)
                    elif method == "logger_print": values["print"]("".join(map(chr,parameters[0])),flush=True)
                    elif method == "logger_flush": module.sys.stdout.flush()
                    elif method == "runner_function":
                        called = module
                        for part in parameters[0].split("."): called = getattr(called,part)
                        values["function"] = called
                    elif method == "runner_member": values[parameters[1]] = getattr(values["self"],parameters[0])
                    elif method == "runner_invoke": values["called"] = values["function"](*[values[key] for key in parameters[0]],**{key:values[slot] for key,slot in parameters[1]})
                    elif method == "runner_label": value = values["model"] in tuple(parameters[0])
                    elif method == "runner_endpoint": values["base"],values["api_key"],values["bearer"] = values["endpoint"]
                    elif method == "runner_keys":
                        values["keys"] = list(getattr(module,parameters[0]))
                        value = [list(map(ord,key)) if type(key) is str else None for key in values["keys"]]
                    elif method == "runner_sorted_keys": value = list(map(ord,", ".join(sorted(values["keys"]))))
                    elif method == "runner_repr": value = list(map(ord,repr(values[parameters[0]])))
                    elif method == "runner_registered": value = values["agent"] in module.RUNNERS
                    elif method == "runner_supported": value = values["scaffold"] in module.WEBBENCH_SCAFFOLDS
                    elif method == "runner_raise": raise RuntimeError("".join(map(chr,parameters[0])))
                    elif method == "runner_construct": values["called"] = module.RUNNERS[values["agent"]](agent=values["agent"],scaffold=values["scaffold"],**values["kwargs"])
                    elif method == "web_compare": values["comparison"] = getattr(operator,parameters[0])(values[parameters[1]],values[parameters[2]])
                    elif method == "web_integer": values[parameters[1]] = int(values[parameters[0]])
                    # The wire codec must not add an int-to-string digit-limit check to arithmetic.
                    elif method == "web_decimal": value = str(Decimal(values[parameters[0]])) if type(values[parameters[0]]) is int else None
                    elif method == "web_integer_literal": values[parameters[0]] = int(Decimal(parameters[1]))
                    elif method == "web_binary": values[parameters[3]] = getattr(operator,parameters[0])(values[parameters[1]],values[parameters[2]])
                    elif method == "web_which": values[parameters[0]] = module.shutil.which(parameters[1])
                    elif method == "web_abort": values["abort"] = module.webbench_undici_abort(values["env"])
                    elif method == "web_uri": values["uri"] = module.Path(module.UNDICI_PRELOAD).as_uri()
                    elif method == "web_stringify": values[parameters[1]] = str(values[parameters[0]])
                    elif method == "web_run": values["completed"] = module.subprocess.run(parameters[0],**parameters[1])
                    elif method == "web_returncode": values["returncode"] = values["completed"].returncode
                    elif method == "stream_list_function": values["function"] = getattr(module,"list",builtins.list)
                    elif method == "stream_set": setattr(values["self"],parameters[0],values[parameters[1]])
                    elif method == "stream_item": values[parameters[2]] = values[parameters[0]][parameters[1]]
                    elif method == "stream_close_function": values["function"] = values["session"].close
                    elif method == "session_get_function": values["function"] = values["queue"].get
                    elif method == "stdout_member": values[parameters[2]] = getattr(values[parameters[0]],parameters[1])
                    elif method == "stdout_iter": values["iterator"] = iter(values["stdout"])
                    elif method == "stdout_next":
                        try: values["line"] = next(values["iterator"])
                        except StopIteration: value = False
                        else: value = True
                    elif method == "stdout_capture_match":
                        pattern = module._ATTEMPT_RE
                        values["match_function"] = pattern.match
                        value = [pattern.pattern,pattern.flags] if type(pattern) is re.Pattern else None
                    elif method == "stdout_match": values["match"] = values["match_function"](values["stripped"])
                    elif method == "stdout_group": values[parameters[1]] = values["match"].group(parameters[0])
                    elif method == "stdout_capture_integer": values["integer_function"] = getattr(module,"int",builtins.int)
                    elif method == "stdout_integer": values["attempt"] = values["integer_function"](values["ordinal"])
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
                    if not deferred_errors and hasattr(module,"urllib") and isinstance(error,module.urllib.error.HTTPError): values["http_error"] = error
                    reply = {"error":{"kind":type(error).__name__,"message":"" if deferred_errors else str(error),"bases":[base.__module__+"."+base.__qualname__ for base in type(error).__mro__]}}
                    if deferred_errors: reply["error"]["details"] = {"provider_error_index":len(errors)-1}
                result = invoke("experiment.swebench.resume",{"job":result["job"],"reply":reply})
            if isinstance(result,dict) and "raised" in result:
                if deferred_errors:
                    index = result.get("details",{}).get("provider_error_index")
                    if index is not None: raise errors[index]
                else:
                    for error in reversed(errors):
                        if type(error).__name__ == result["raised"] and str(error) == result["message"]: raise error
            result = checked(result)
            if isinstance(result,dict) and "provider_error" in result: raise RuntimeError(decode(result["provider_error"]))
            if isinstance(result,dict) and "provider_ref" in result: return values[result["provider_ref"]]
            if isinstance(result,dict) and "provider_points" in result: return "".join(map(chr,result["provider_points"]))
            if isinstance(result,dict) and "provider_encoded" in result: result = json.loads(result["provider_encoded"])
            return tuple(result) if name in ("verify_model","anthropic_optimizer_call","optimizer_call") else result
        native._te_provider_operation = agent,name
        return native
    def runner_calls(owner,domain,classes):
        if owner is None or not hasattr(owner,"RUNNERS"): return
        for cls_name,agent in zip(classes,("claude","copilot","codex")):
            cls = getattr(owner,cls_name)
            for method,operation in (("optimizer_call","runner_optimizer_"+domain),("_preflight_optimizer_endpoint","runner_preflight")):
                if method not in cls.__dict__: continue
                original = getattr(cls,method)
                if getattr(original,"_te_provider_operation",None) == (agent,operation): continue
                replacements[original] = function(owner,original,agent,operation)
                setattr(cls,method,replacements[original])
        original = owner.make_runner
        operation = "runner_factory_"+domain
        if getattr(original,"_te_provider_operation",None) != ("claude",operation):
            replacements[original] = function(owner,original,"claude",operation)
            owner.make_runner = replacements[original]
        if domain == "webbench":
            for name in ("next_event","next_request","_read_stdout"):
                original = getattr(owner._EvalSession,name)
                if getattr(original,"_te_provider_operation",None) == ("webbench",name): continue
                replacements[original] = function(owner,original,"webbench",name)
                setattr(owner._EvalSession,name,replacements[original])
            for name in ("begin_stream","end_stream","session_for","close_session"):
                original = getattr(owner.WebBenchRunner,name)
                if getattr(original,"_te_provider_operation",None) == ("webbench",name): continue
                replacements[original] = function(owner,original,"webbench",name)
                setattr(owner.WebBenchRunner,name,replacements[original])
            for name in ("webbench_repo","node_bin","webbench_undici_abort","webbench_task_timeout","webbench_undici_env"):
                original = getattr(owner,name)
                if getattr(original,"_te_provider_operation",None) == ("webbench",name): continue
                replacements[original] = function(owner,original,"webbench",name)
                setattr(owner,name,replacements[original])
            original = owner.CodexWebBenchRunner._probe_namespace_guard
            if getattr(original,"_te_provider_operation",None) != ("webbench","_probe_namespace_guard"):
                replacements[original] = function(owner,original,"webbench","_probe_namespace_guard")
                owner.CodexWebBenchRunner._probe_namespace_guard = replacements[original]
    if runner_module is not None:
        runner_calls(runner_module,"webbench",("WebBenchRunner","CopilotWebBenchRunner","CodexWebBenchRunner"))
        return replacements
    for module,agent,names in ((swebench,"claude",("resolve_anthropic_key","agent_credential_env","preflight_optimizer_endpoint","anthropic_optimizer_call","optimizer_log_path","_cell_of","set_optimizer_context","optimizer_call_kind","_log_optimizer_call","_log")),
                              (copilot,"copilot",("auth_env","byok_active","provider_base","anthropic_optimizer_endpoint","provider_env","provider_secret_names","normalize_model","effort_applies","validate_priceable","optimizer_call","copilot_bin","cli_version","require_install","run_env")),
                              (codex,"codex",("resolve_codex_routing","validate_priceable","fetch_catalog","verify_model","preflight_optimizer","optimizer_call","_responses_text"))):
        for name in names:
            original = getattr(module,name)
            replacements[original] = function(module,original,agent,name)
            setattr(module,name,replacements[original])
    webbench = sys.modules.get("te_harness.webbench")
    if webbench is not None:
        for name in ("resolve_anthropic_key","agent_credential_env","preflight_optimizer_endpoint","anthropic_optimizer_call","set_optimizer_context","_cell_of","_log"):
            original = getattr(webbench,name,None)
            if original in replacements: setattr(webbench,name,replacements[original])
    for owner,domain,classes in ((swebench,"swebench",("ClaudeSwebenchRunner","CopilotSwebenchRunner","CodexSwebenchRunner")),
                                 (webbench,"webbench",("WebBenchRunner","CopilotWebBenchRunner","CodexWebBenchRunner"))):
        runner_calls(owner,domain,classes)
    return replacements
