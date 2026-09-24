#!/usr/bin/env python3
import json,sys
obj=json.load(open(sys.argv[1],encoding='utf-8'))
open(sys.argv[2],'w',encoding='utf-8').write(json.dumps(obj,sort_keys=True,separators=(',',':'))+'\n')
