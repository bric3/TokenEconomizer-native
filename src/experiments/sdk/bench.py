"""Private bootstrap for the native benchmark command."""
import sys
from te_runtime.driver import Driver
from te_runtime.transport import Bridge

engine,root,program,*arguments=sys.argv[1:]
sys.argv=[program,*arguments]
with Bridge(engine) as bridge:
    Driver(bridge,root).main()
