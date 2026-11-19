"""Adapt the counted validation channels to the production SDK bridge interface."""
import contextlib

from native_plugin import invoke
from native_cleanup_transport import isolated_invoke


class ValidationBridge:
    def __init__(self,request=invoke): self.request=request
    frames=staticmethod(contextlib.nullcontext)
    callback_scope=staticmethod(contextlib.nullcontext)
    deliver_signals=staticmethod(lambda:None)

    @contextlib.contextmanager
    def isolated(self):
        with isolated_invoke() as request: yield ValidationBridge(request)
