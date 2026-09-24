#!/usr/bin/env python3
import json,sys
for c in json.load(open(sys.argv[1],encoding='utf-8'))['cases']:
    print(c['id'],c['expected_error'],c['path'])
