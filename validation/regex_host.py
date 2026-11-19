"""Pinned detector regex oracle and native protocol binding; no matching policy here."""
import json
from pathlib import Path
import sys

ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/"reference/upstream"))
from tokeneconomizer.microskills import distiller
if "--native" in sys.argv:
    from native_plugin import invoke

for line in sys.stdin:
    request=json.loads(line)
    try:
        mode=request.get("mode","regex")
        if mode == "detector":
            spec,digest=request["spec"],request["digest"]
            if "--native" in sys.argv:
                arguments=dict(spec=spec)
                if "features" in request: arguments["features"]=request["features"]
                snapshot=invoke("detector.new",arguments)
                value=invoke("detector.verdict",dict(self=snapshot,dg=digest))
            else:
                detector=distiller.DetectorV3Scope(spec,request["features"]) if "features" in request else distiller.Detector(spec)
                snapshot=dict(scope=detector.scope,confirm=None if detector.confirm is None else detector.confirm.pattern,
                    contra=None if detector.contra is None else detector.contra.pattern,fired=detector.fired,raw=detector.raw,
                    features=getattr(detector,"features",None))
                value=detector.verdict(digest)
            response=dict(snapshot=snapshot,value=value)
        elif mode == "extractor":
            spec,commands=request["spec"],request["commands"]
            if "--native" in sys.argv:
                snapshot=invoke("extractor.new",dict(spec=spec))
                value=invoke("extractor.compute",dict(self=snapshot,cmds=commands))
            else:
                extractor=distiller.Extractor(spec)
                snapshot=dict(name=extractor.name,kind=extractor.kind,rx=None if extractor.rx is None else extractor.rx.pattern,spec=extractor.spec)
                value=extractor.compute(commands)
            response=dict(snapshot=snapshot,value=value)
        else:
            pattern=request["pattern"]
            if "--native" in sys.argv:
                compiled=invoke("distiller._rx",dict(pat=pattern))
                matches=[invoke("distiller.regex_search",dict(pattern=pattern,text=text)) for text in request["texts"]]
            else:
                compiled=distiller._rx(pattern)
                matches=[bool(compiled and compiled.search(text)) for text in request["texts"]]
                compiled=None if compiled is None else compiled.pattern
            response=dict(compiled=compiled,matches=matches)
    except BaseException as error: response=dict(raised=type(error).__name__,message=str(error))
    print(json.dumps(response,sort_keys=True,ensure_ascii=True),flush=True)
