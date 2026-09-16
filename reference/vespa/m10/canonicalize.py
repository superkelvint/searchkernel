#!/usr/bin/env python3
import json,sys
obj=json.load(sys.stdin)
# Object key order and match-feature declaration order are deliberately not semantic.
print(json.dumps(obj,sort_keys=True,separators=(",",":")))
