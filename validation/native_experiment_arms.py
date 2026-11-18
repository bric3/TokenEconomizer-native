"""Python data-object binding for the native arm registry."""
from native_plugin import invoke


def install(module):
    from tokeneconomizer.config import EconomizerConfig, InjectionConfig, ToolingConfig, SpiralConfig
    def factory(name):
        def config():
            value=invoke("experiment.arms.config",{"name":name})
            return EconomizerConfig(InjectionConfig(**value["injection"]),ToolingConfig(**value["tooling"]),SpiralConfig(**value["spiral"]))
        return config
    registry=invoke("experiment.arms.registry",{})
    module.ARM_ORDER=tuple(registry["order"])
    module.ARMS.clear()
    module.ARMS.update({name:module.Arm(value["mode"],factory(name),value["env"],value["scaffold"]) for name,value in registry["arms"].items()})
    module.SCAFFOLD_ARMS.clear()
    module.SCAFFOLD_ARMS.update(registry["scaffolds"])
