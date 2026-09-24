#!/usr/bin/env python3
import json, sys
for path in sys.argv[1:3]:
    with open(path, encoding="utf-8") as f: data=json.load(f)
    data["artifact_generation"]=int(sys.argv[3]); data["adapter_static_link_closure"]=True
    with open(path,"w",encoding="utf-8") as f: json.dump(data,f,indent=2,sort_keys=True); f.write("\n")
