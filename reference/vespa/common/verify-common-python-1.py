#!/usr/bin/env python3
import json
import sys

with open(sys.argv[1], encoding="utf-8") as stream:
    pin = json.load(stream)
print(pin["commit"])
