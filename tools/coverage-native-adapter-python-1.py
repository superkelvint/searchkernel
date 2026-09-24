#!/usr/bin/env python3
import pathlib
import re
import sys

report = open(sys.argv[1], encoding="utf-8").read()
sources = {path.name for path in pathlib.Path(sys.argv[2]).glob("*.cc")}
sections = re.split(r"(?m)^File ['\"]", report)[1:]
reported_sources = set()
line_rates = []
branch_rates = []
function_rates = []
for section in sections:
    name, _, body = section.partition("'\n")
    if name.rsplit("/", 1)[-1] not in sources:
        continue
    reported_sources.add(name.rsplit("/", 1)[-1])
    lines = re.search(r"Lines executed:([0-9.]+)% of ([0-9]+)", body)
    branches = re.search(r"Branches executed:([0-9.]+)% of ([0-9]+)", body)
    functions = re.search(r"Functions executed:([0-9.]+)% of ([0-9]+)", body)
    if lines:
        line_rates.append((float(lines.group(1)), int(lines.group(2))))
    if branches:
        branch_rates.append((float(branches.group(1)), int(branches.group(2))))
    if functions:
        function_rates.append((float(functions.group(1)), int(functions.group(2))))
if not line_rates:
    raise SystemExit("gcov did not report the split native adapter sources")
missing = sorted(sources - reported_sources)
if missing:
    raise SystemExit(
        "gcov did not report all native adapter translation units: "
        + ", ".join(missing)
    )
def average(values):
    if not values:
        return "unknown"
    rate, total = zip(*values)
    return f"{sum(value * count for value, count in values) / sum(total):.2f}"
print(average(line_rates), average(branch_rates), average(function_rates))
