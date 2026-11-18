"""Data-object binding for arm-registry comparisons."""
import dataclasses
import json
from pathlib import Path
import sys

ROOT=Path(__file__).resolve().parents[1]
sys.path[:0]=[str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
from te_bench import arms
if "--native" in sys.argv:
    from native_experiment_arms import install
    install(arms)

for line in sys.stdin:
    request=json.loads(line)
    name=request.get("name")
    if request.get("operation") == "mutate":
        value=arms.ARMS[name].cfg()
        value.injection.profile="changed"
        value.injection.propose_every=-1
        value.tooling.evidence=-1
        value.spiral.codex_spiral_profile="changed"
    output={"order":arms.ARM_ORDER,"names":list(arms.ARMS),"scaffolds":arms.SCAFFOLD_ARMS,
            "arms":{key:dict(mode=arm.mode,config=dataclasses.asdict(arm.cfg()),env=arm.env,scaffold=arm.scaffold)
                    for key,arm in arms.ARMS.items()}}
    print(json.dumps(output,sort_keys=True,separators=(",",":")),flush=True)
