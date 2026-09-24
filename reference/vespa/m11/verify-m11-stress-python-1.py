#!/usr/bin/env python3
import json
import sys

actual = json.load(open(sys.argv[1], encoding="utf-8"))
limits = json.load(open(sys.argv[2], encoding="utf-8"))
if actual["final_fds"] > actual["baseline_fds"] + limits["fd_slack"]:
    raise SystemExit("file descriptor count grew beyond the frozen guardrail")
if actual["final_threads"] > actual["baseline_threads"] + 1:
    raise SystemExit("native thread count grew beyond the frozen guardrail")
allowed_pss = max(
    limits["pss_absolute_slack_bytes"] // 1024,
    int(actual["baseline_pss_kib"] * (1 + limits["pss_relative_slack"])),
)
if actual["final_pss_kib"] > actual["baseline_pss_kib"] + allowed_pss:
    raise SystemExit("PSS grew beyond the frozen guardrail")
if actual["final_files"] > actual["baseline_files"] + limits["disk_file_slack"]:
    raise SystemExit("read-only reopen created too many files")
if actual["final_bytes"] > actual["baseline_bytes"] + limits["disk_byte_slack"]:
    raise SystemExit("read-only reopen grew on-disk state beyond the guardrail")
