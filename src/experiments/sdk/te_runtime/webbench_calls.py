"""WebBench descriptors and method mappings; coordination policy executes in Rust."""
import inspect
from .providers import bind as provider_bindings


def bind(bridge,module):
    replacements={}
    runtime=provider_bindings(bridge,shared=module)
    def function(owner,original,agent,name):
        native=runtime(owner,original,agent,name)
        native._te_provider_operation=agent,name
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
        cleanup = "_cleanup_sessions" if domain == "webbench" else "_cleanup_containers"
        for name,operation in ((cleanup,cleanup.removeprefix("_")),("_install_signal_cleanup","signal_install")):
            original = getattr(owner,name)
            if getattr(original,"_te_provider_operation",None) == (domain,operation): continue
            replacements[original] = function(owner,original,domain,operation)
            setattr(owner,name,replacements[original])
        if domain == "webbench":
            for cls_name,methods in (
                ("WebBenchRunner",(("scaffold_profile_for","profile_claude"),("_scaffold_context","context"),("_apply_scaffold","apply"),("scaffold_answer_excludes","excludes"))),
                ("CopilotWebBenchRunner",(("scaffold_profile_for","profile_copilot"),)),
                ("CodexWebBenchRunner",(("scaffold_profile_for","profile_codex"),)),
            ):
                cls = getattr(owner,cls_name)
                for name,suffix in methods:
                    original = getattr(cls,name)
                    operation = "scaffold_"+suffix
                    if getattr(original,"_te_provider_operation",None) == ("webbench",operation): continue
                    replacements[original] = function(owner,original,"webbench",operation)
                    setattr(cls,name,replacements[original])
            for cls_name,methods in (
                ("WebBenchRunner",(("_net_guard_log","log"),("_net_guard_blocks","blocks"),("net_guard_settings_path","settings_claude"),("net_guard_hook_entry","entry_claude"),("_install_net_guard","install"),("_verify_net_guard","verify"))),
                ("CopilotWebBenchRunner",(("net_guard_settings_path","settings_copilot"),("net_guard_hook_entry","entry_copilot"))),
                ("CodexWebBenchRunner",(("_hosts_file","hosts"),("net_guard_settings_path","settings_codex"),("_install_net_guard","install_codex"))),
            ):
                cls = getattr(owner,cls_name)
                for name,suffix in methods:
                    descriptor = inspect.getattr_static(cls,name)
                    original = descriptor.__func__ if isinstance(descriptor,staticmethod) else getattr(cls,name)
                    operation = "network_"+suffix
                    if getattr(original,"_te_provider_operation",None) == ("webbench",operation): continue
                    replacement = function(owner,original,"webbench",operation)
                    replacements[original] = replacement
                    setattr(cls,name,staticmethod(replacement) if isinstance(descriptor,staticmethod) else replacement)
            for cls_name,methods in (
                ("WebBenchRunner",(("agent_home","home_claude"),("scaffold_bin_dir","bin_dir"),("scaffold_exec_env","exec_env_claude"),("prepare_agent_home","prepare_claude"),("_merge_segments","merge_claude"),("_segment_usage","usage_claude"),("_row_from_segment","row_claude"))),
                ("CopilotWebBenchRunner",(("scaffold_exec_env","exec_env_copilot"),("prepare_agent_home","prepare_copilot"),("scaffold_copilot_home","copilot_home"),("_merge_segments","merge_copilot"),("_segment_usage","usage_copilot"),("_row_from_segment","row_copilot"))),
                ("CodexWebBenchRunner",(("agent_home","home_codex"),("scaffold_exec_env","exec_env_codex"),("_write_config","config_codex"),("_merge_segments","merge_codex"),("_segment_usage","usage_codex"),("_row_from_segment","row_codex"))),
            ):
                cls = getattr(owner,cls_name)
                for name,suffix in methods:
                    descriptor = inspect.getattr_static(cls,name)
                    original = descriptor.__func__ if isinstance(descriptor,staticmethod) else getattr(cls,name)
                    operation = "agent_"+suffix
                    if getattr(original,"_te_provider_operation",None) == ("webbench",operation): continue
                    replacement = function(owner,original,"webbench",operation)
                    replacements[original] = replacement
                    setattr(cls,name,staticmethod(replacement) if isinstance(descriptor,staticmethod) else replacement)
            for cls_name,operation in (("WebBenchRunner","segment_claude"),("CopilotWebBenchRunner","segment_copilot"),("CodexWebBenchRunner","segment_codex")):
                cls = getattr(owner,cls_name)
                original = cls._segment
                if getattr(original,"_te_provider_operation",None) == ("webbench",operation): continue
                replacements[original] = function(owner,original,"webbench",operation)
                cls._segment = replacements[original]
            for cls_name,method,operation in (("WebBenchRunner","_write_run_config","write_run_config"),("WebBenchRunner","_make_guard","guard_claude"),("CodexWebBenchRunner","_make_guard","guard_codex")):
                cls = getattr(owner,cls_name)
                original = getattr(cls,method)
                if getattr(original,"_te_provider_operation",None) == ("webbench",operation): continue
                replacements[original] = function(owner,original,"webbench",operation)
                setattr(cls,method,replacements[original])
            for name,operation in (("build_prompt","build_prompt"),("strip_transport_instructions","strip_transport"),("_session_log_text_host","session_log"),("_shq","network_quote")):
                original = getattr(owner,name)
                if getattr(original,"_te_provider_operation",None) == ("webbench",operation): continue
                replacements[original] = function(owner,original,"webbench",operation)
                setattr(owner,name,replacements[original])
            for cls_name,method,operation in (("CopilotWebBenchRunner","run_task","copilot_task"),("CodexWebBenchRunner","run_task","codex_task"),("CodexWebBenchRunner","_prune_agent_home","prune_home")):
                cls = getattr(owner,cls_name)
                original = getattr(cls,method)
                if getattr(original,"_te_provider_operation",None) == ("webbench",operation): continue
                replacements[original] = function(owner,original,"webbench",operation)
                setattr(cls,method,replacements[original])
            for name,operation in (("_load_ignore_globs","load_ignore_globs"),("_ancestors","ancestors"),("_ignored","ignored")):
                original = getattr(owner,name)
                if getattr(original,"_te_provider_operation",None) == ("webbench",operation): continue
                replacements[original] = function(owner,original,"webbench",operation)
                setattr(owner,name,replacements[original])
            for name,operation in (("run_task","run_task"),("_materialize","materialize"),("_collect","collect")):
                original = getattr(owner.WebBenchRunner,name)
                if getattr(original,"_te_provider_operation",None) == ("webbench",operation): continue
                replacements[original] = function(owner,original,"webbench",operation)
                setattr(owner.WebBenchRunner,name,replacements[original])
            for name,operation in (("do_POST","http_post"),("log_message","http_log_message")):
                original = getattr(owner._AgentHandler,name)
                if getattr(original,"_te_provider_operation",None) == ("webbench",operation): continue
                replacements[original] = function(owner,original,"webbench",operation)
                setattr(owner._AgentHandler,name,replacements[original])
            original = owner._EvalSession.__init__
            if getattr(original,"_te_provider_operation",None) != ("webbench","eval_start"):
                replacements[original] = function(owner,original,"webbench","eval_start")
                owner._EvalSession.__init__ = replacements[original]
            original = owner._EvalSession.close
            if getattr(original,"_te_provider_operation",None) != ("webbench","eval_close"):
                replacements[original] = function(owner,original,"webbench","eval_close")
                owner._EvalSession.close = replacements[original]
            for name in ("next_event","next_request","_read_stdout","_full_task_ids"):
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
    runner_calls(module,"webbench",("WebBenchRunner","CopilotWebBenchRunner","CodexWebBenchRunner"))
    return replacements
