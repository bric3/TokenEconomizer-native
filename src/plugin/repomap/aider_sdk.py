"""I/O adapter for the optional aider SDK; selection and configuration belong to Rust."""
import contextlib
import json
import sys

request = json.load(sys.stdin)
with contextlib.redirect_stdout(sys.stderr):
    from aider.io import InputOutput
    from aider.models import Model
    from aider.repomap import RepoMap

    if request["cache"]:
        RepoMap.TAGS_CACHE_DIR = request["cache"]
    io = InputOutput()
    model = Model("gpt-4o")
    mapper = RepoMap(map_tokens=int(request["budget"]), root=request["repo"],
                     main_model=model, io=io, verbose=False)
    text = mapper.get_repo_map([], request["files"], mentioned_idents=set(request["idents"]))
sys.stdout.write(text or "")
