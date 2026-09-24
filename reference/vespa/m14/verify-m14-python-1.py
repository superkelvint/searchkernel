#!/usr/bin/env python3
import json,sys
for case in json.load(open(sys.argv[1],encoding='utf-8'))['cases']:
    print(case['id'],case['path'],case['expected'])
