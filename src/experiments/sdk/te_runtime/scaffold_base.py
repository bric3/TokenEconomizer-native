"""Scaffold classes and caller-owned callbacks; scaffold operations execute in Rust."""
import hashlib
import json
import os
import shlex
from types import ModuleType

from .providers import bind as provider_bindings


def bind(bridge):
    module=ModuleType(__name__)
    vars(module).update(hashlib=hashlib,json=json,os=os,shlex=shlex)
    function=provider_bindings(bridge)
    def operation(name):
        def declare(signature):
            signature.__qualname__=signature.__qualname__.removeprefix("bind.<locals>.")
            return function(module,signature,"scaffold",name)
        return declare

    class ScaffoldError(RuntimeError): pass

    class AgentProfile:
        @operation("profile_new")
        def __init__(self,kind,home,instructions_file,nav_tools,mcp_args,
                     patch_excludes=(),bin_dir="/usr/local/bin",env=None): pass
        @operation("profile_mcp")
        def mcp_args(self,cfg_path): pass

    class ScaffoldContext:
        @operation("context_new")
        def __init__(self,*,profile,container,task,run_dir,exec_,copy_in,host_run,
                     read_bytes=None,log=None): pass
        @operation("context_read")
        def read_bytes(self,script,timeout=600): pass
        @operation("context_log")
        def log(self,msg): pass
        @operation("instructions")
        def write_instructions(self,text): pass
        @operation("mcp_config")
        def write_mcp_config(self,servers,filename="scaffold.mcp.json"): pass
        @operation("merge_json")
        def merge_json_file(self,path,patch): pass

    class Scaffold:
        name="base"
        patch_excludes=()
        @operation("mounts")
        def mounts(self,profile): pass
        @operation("preflight")
        def preflight(self,profile): pass
        @operation("setup")
        def setup(self,ctx): pass

    vars(module).update(ScaffoldError=ScaffoldError,AgentProfile=AgentProfile,
        ScaffoldContext=ScaffoldContext,Scaffold=Scaffold)
    return module
