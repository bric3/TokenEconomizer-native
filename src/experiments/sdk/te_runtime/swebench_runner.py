"""Runner descriptors and object bindings; coordination policy executes in Rust."""
from .providers import bind as provider_bindings


def bind(bridge,module,*,container_helpers=None):
    function=provider_bindings(bridge,shared=module,container_helpers=container_helpers)
    replacements={}
    def method(owner,name,agent,operation):
        original=getattr(owner,name)
        if getattr(original,"_te_provider_operation",None) == (agent,operation): return
        native=function(module,original,agent,operation)
        native._te_provider_operation=agent,operation
        replacements[original]=native
        setattr(owner,name,native)
    for cls,agent in ((module.ClaudeSwebenchRunner,"claude"),
                      (module.CopilotSwebenchRunner,"copilot"),
                      (module.CodexSwebenchRunner,"codex")):
        method(cls,"_segment","swebench","segment_"+agent)
        method(cls,"_merge_segments","webbench","agent_merge_"+agent)
        for name,operation in (("optimizer_call","runner_optimizer_swebench"),
                               ("_preflight_optimizer_endpoint","runner_preflight")):
            if name in vars(cls): method(cls,name,agent,operation)
    for name,agent,operation in (("_scaffold_context","swebench","scaffold_context"),
                                 ("_session_log_text","swebench","session_log")):
        method(module.ClaudeSwebenchRunner,name,agent,operation)
    for name,agent,operation in (("make_runner","claude","runner_factory_swebench"),
                                 ("_cleanup_containers","swebench","cleanup_containers"),
                                 ("_install_signal_cleanup","swebench","signal_install")):
        method(module,name,agent,operation)
    return replacements
