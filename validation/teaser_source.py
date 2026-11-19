"""I/O entry point for the unchanged teaser, with outputs in an owned directory."""
import importlib.util
from pathlib import Path
import sys

assets = Path(__file__).resolve().parents[1] / "reference/upstream/docs/assets"
sys.path.insert(0, str(assets))
spec = importlib.util.spec_from_file_location("source_teaser", assets / "teaser.py")
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)
fonts, out, mode = sys.argv[1:]
if mode == "poster":
    module.L = module.page_look(module.plex(fonts))
    module.still(2, Path(out) / "teaser.png")
else:
    sys.argv = ["teaser.py", "--plex", fonts]
    if mode == "page":
        sys.argv += ["--page", out]
    elif mode == "gif":
        module.OUT_DIR = Path(out)
    else:
        raise ValueError("unknown source teaser operation")
    module.main()
