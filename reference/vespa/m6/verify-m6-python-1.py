#!/usr/bin/env python3
import json, sys
left=json.load(open(sys.argv[1])); right=json.load(open(sys.argv[2]))
assert left['fields'] == right['fields'], 'identity pair field models unexpectedly differ'
assert left['document_type'] == 'typea', left
assert right['document_type'] == 'products', right
