"""I/O adapter for source merge programs and native merge context execution."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT=Path(__file__).resolve().parents[1]
native="--native" in sys.argv
if "--sdk" in sys.argv:
    from standalone_scaffolds import base
    ScaffoldContext=base.ScaffoldContext
else:
    sys.path[:0]=[str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
    from te_bench.baselines.base import ScaffoldContext
    if native:
        from te_bench.baselines import base
        from native_experiment_providers import install
        install(scaffold_module=base)

for line in sys.stdin:
    request=json.loads(line);directory=Path(request["directory"]);captured=directory/("native-merge.sh" if native else "source-merge.py");completed=[]
    def copy(source,destination):shutil.copyfile(source,captured);return subprocess.CompletedProcess([],0)
    def execute(script,timeout):
        result=subprocess.run(["bash" if native else sys.executable,str(captured)],cwd=directory,capture_output=True,text=True,timeout=timeout,env=dict(os.environ,PYTHONINTMAXSTRDIGITS=str(request.get("int_max_digits",4300))))
        completed.append(result);return result
    try:
        sys.set_int_max_str_digits(request.get("int_max_digits",4300))
        patch=json.loads(Path(request["patch"]).read_text(encoding="utf-8"))
        context=ScaffoldContext(profile=None,container=None,task={},run_dir=str(directory),exec_=execute,copy_in=copy,host_run=None)
        merged=context.merge_json_file(request["path"],patch)
        result=completed[-1];lines=result.stderr.strip().splitlines()
        if native:
            assert merged == ("merged" in result.stdout)
            assert captured.read_text(encoding="utf-8").startswith("exec ") and " --merge-json " in captured.read_text(encoding="utf-8")
        output={"code":result.returncode,"error":lines[-1] if lines else "","stdout":result.stdout}
    except BaseException as error:output={"code":1,"error":type(error).__name__+": "+str(error),"stdout":""}
    print(json.dumps(output,sort_keys=True,separators=(",",":"),ensure_ascii=False),flush=True)
