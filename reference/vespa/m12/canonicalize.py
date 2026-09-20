#!/usr/bin/env python3
import json,sys
obj=json.load(sys.stdin)
# M12 freezes ordered hit IDs, not score bits or total-hit count.
print(json.dumps(obj,sort_keys=True,separators=(",",":")))
