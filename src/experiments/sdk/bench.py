"""Private bootstrap for the native benchmark command."""
import sys
from te_runtime.transport import Bridge

engine,root,program,*arguments=sys.argv[1:]
sys.argv=[program,*arguments]
with Bridge(engine) as bridge:
    if arguments[:1] == ["export-swebench"]:
        from te_runtime.datasets import Exporter
        Exporter(bridge).main(program + " export-swebench", arguments[1:])
    else:
        from te_runtime.driver import Driver
        Driver(bridge,root).main()
