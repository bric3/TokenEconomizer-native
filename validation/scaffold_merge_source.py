"""I/O adapter that executes the original scaffold's generated merge program."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT=Path(__file__).resolve().parents[1]
sys.path[:0]=[str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
from te_bench.baselines.base import ScaffoldContext

for line in sys.stdin:
    request=json.loads(line);directory=Path(request["directory"]);captured=directory/"source-merge.py";completed=[]
    def copy(source,destination):shutil.copyfile(source,captured);return subprocess.CompletedProcess([],0)
    def execute(script,timeout):
        result=subprocess.run([sys.executable,str(captured)],cwd=directory,capture_output=True,text=True,timeout=timeout,env=dict(os.environ,PYTHONINTMAXSTRDIGITS=str(request.get("int_max_digits",4300))))
        completed.append(result);return result
    try:
        sys.set_int_max_str_digits(request.get("int_max_digits",4300))
        patch=json.loads(Path(request["patch"]).read_text(encoding="utf-8"))
        context=ScaffoldContext(profile=None,container=None,task={},run_dir=str(directory),exec_=execute,copy_in=copy,host_run=None)
        context.merge_json_file(request["path"],patch)
        result=completed[-1];lines=result.stderr.strip().splitlines()
        output={"code":result.returncode,"error":lines[-1] if lines else "","stdout":result.stdout}
    except BaseException as error:output={"code":1,"error":type(error).__name__+": "+str(error),"stdout":""}
    print(json.dumps(output,sort_keys=True,separators=(",",":"),ensure_ascii=False),flush=True)
