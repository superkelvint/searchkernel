#!/usr/bin/env python3
import json,sys
exp=json.load(open(sys.argv[1])); got=json.load(open(sys.argv[2]));
if got['id']!=exp['id'] or got['schema']!=exp['schema']: raise SystemExit('identity mismatch')
if got['typed']!=exp['query']: raise SystemExit(f"typed canonical mismatch: {got['typed']} != {exp['query']}")
if got['yql']!=exp['query']: raise SystemExit(f"YQL canonical mismatch: {got['yql']} != {exp['query']}")
