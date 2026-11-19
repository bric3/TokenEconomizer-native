"""Guard descriptors and method signatures; registration policy executes in Rust."""
import inspect

from .providers import bind as provider_bindings


def bind(bridge,module):
    function=provider_bindings(bridge)
    replacements={}
    for cls,methods in (
        (module.ClaudeSwebenchRunner,(("NET_GUARD_SCRIPT","script"),("NET_GUARD_LOG","log"),
            ("_answer_key_attempts","attempts"),("_net_guard_merge_program","program"),
            ("_write_hook_entry","write"),("_register_net_guard","register_claude"))),
        (module.CopilotSwebenchRunner,(("_register_net_guard","register_copilot"),)),
        (module.CodexSwebenchRunner,(("_register_net_guard","register_codex"),)),
    ):
        for name,operation in methods:
            descriptor=inspect.getattr_static(cls,name)
            original=descriptor.fget if isinstance(descriptor,property) else descriptor.__func__ if isinstance(descriptor,staticmethod) else descriptor
            native=function(module,original,"swebench","guard_"+operation)
            replacements[original]=native
            if isinstance(descriptor,property): native=property(native,descriptor.fset,descriptor.fdel,descriptor.__doc__)
            elif isinstance(descriptor,staticmethod): native=staticmethod(native)
            setattr(cls,name,native)
    return replacements
