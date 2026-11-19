"""Scaffold types and external-library I/O; selection, setup and cleanup run in Rust."""
import io
import json
import os
import shlex
import shutil
import subprocess
import sys
import tarfile
from types import ModuleType, SimpleNamespace

from .providers import bind as provider_bindings
from .scaffold_base import bind as base_bindings
from .transport import checked


def bind(bridge):
    base=base_bindings(bridge)
    rtk,serena,repomap,registry=(ModuleType(__name__+"."+name) for name in ("rtk","serena","repomap","registry"))
    constants=checked(bridge.request("experiment.scaffolds.constants",{}))
    for name,module in (("rtk",rtk),("serena",serena),("repomap",repomap)):
        vars(module).update(constants[name],io=io,json=json,os=os,shlex=shlex,shutil=shutil,
            subprocess=subprocess,sys=sys,tarfile=tarfile,Scaffold=base.Scaffold,ScaffoldError=base.ScaffoldError)
    serena.SERENA_TOOLS=tuple(serena.SERENA_TOOLS)
    function=provider_bindings(bridge)
    def operation(module,name):
        def declare(signature):
            signature.__qualname__=signature.__qualname__.removeprefix("bind.<locals>.")
            return function(module,signature,"scaffold",name)
        return declare

    @operation(rtk,"rtk_cache")
    def cache_dir(): pass
    rtk.cache_dir=cache_dir

    class RtkScaffold(base.Scaffold):
        @staticmethod
        @operation(rtk,"rtk_hook_binary")
        def _hook_binary(profile): pass
        @classmethod
        @operation(rtk,"rtk_claude_hook")
        def _claude_hook(cls,profile): pass
        @classmethod
        @operation(rtk,"rtk_copilot_config")
        def _copilot_hook_config(cls,profile): pass
        @operation(rtk,"rtk_bin_path")
        def bin_path(self): pass
        @operation(rtk,"rtk_preflight")
        def preflight(self,profile): pass
        @operation(rtk,"rtk_stage")
        def stage_host_binary(self,ctx): pass
        @operation(rtk,"rtk_setup")
        def setup(self,ctx): pass
        @operation(rtk,"rtk_codex")
        def _codex_instructions(self,ctx): pass
        @operation(rtk,"rtk_copilot")
        def _install_copilot_hook(self,ctx): pass
        @operation(rtk,"rtk_probe")
        def _probe_hook_rewrites(self,ctx,agent): pass

    class SerenaScaffold(base.Scaffold):
        @operation(serena,"serena_setup")
        def setup(self,ctx): pass
        @staticmethod
        @operation(serena,"serena_server")
        def mcp_server(profile): pass
        @staticmethod
        @operation(serena,"serena_nudge")
        def nudge(profile): pass

    @operation(repomap,"repomap_tokens")
    def map_tokens(): pass
    @operation(repomap,"repomap_idents")
    def extract_idents(text): pass
    repomap.map_tokens,repomap.extract_idents=map_tokens,extract_idents

    class RepoMapScaffold(base.Scaffold):
        @operation(repomap,"repomap_preflight")
        def preflight(self,profile): pass
        @operation(repomap,"repomap_fetch")
        def fetch_sources(self,ctx): pass
        @operation(repomap,"repomap_generate")
        def generate(self,src_dir,idents): pass
        @operation(repomap,"repomap_setup")
        def setup(self,ctx): pass

    for module,cls in ((rtk,RtkScaffold),(serena,SerenaScaffold),(repomap,RepoMapScaffold)):
        for name,value in vars(module).pop("class").items():
            setattr(cls,name,tuple(value) if name == "patch_excludes" else value)
        setattr(module,cls.__name__,cls)
    vars(registry).update(Scaffold=base.Scaffold,ScaffoldContext=base.ScaffoldContext,
        AgentProfile=base.AgentProfile,ScaffoldError=base.ScaffoldError,
        RtkScaffold=RtkScaffold,SerenaScaffold=SerenaScaffold,RepoMapScaffold=RepoMapScaffold,
        SCAFFOLDS={"rtk":RtkScaffold,"serena":SerenaScaffold,"repomap":RepoMapScaffold},
        ARM_SCAFFOLDS=checked(bridge.request("experiment.arms.registry",{}))["scaffolds"])
    @operation(registry,"registry_get")
    def get(name): pass
    @operation(registry,"registry_for_arm")
    def for_arm(arm): pass
    registry.get,registry.for_arm=get,for_arm
    return SimpleNamespace(base=base,rtk=rtk,serena=serena,repomap=repomap,registry=registry)
