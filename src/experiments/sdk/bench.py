"""Private bootstrap for the native benchmark command."""
import sys
from te_runtime.transport import Bridge

engine,root,program,*arguments=sys.argv[1:]
sys.argv=[program,*arguments]
with Bridge(engine) as bridge:
    if arguments[:1] == ["capture-catalog"]:
        from te_runtime.catalog import main
        main(bridge, root)
    elif arguments[:1] in (["export-swebench"], ["webbench-from-yaml"]):
        from te_runtime.datasets import Exporter
        exporter = Exporter(bridge)
        entry = exporter.main if arguments[0] == "export-swebench" else exporter.yaml_main
        entry(program + " " + arguments[0], arguments[1:])
    else:
        from te_runtime.driver import Driver
        Driver(bridge,root).main()
