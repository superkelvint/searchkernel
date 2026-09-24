#!/usr/bin/env python3
import json
import os
import re
import sys
import urllib.error
import urllib.request

build_env, target, commit = sys.argv[1:]
repo = os.environ.get("SEARCHKERNEL_GITHUB_REPO", "superkelvint/searchkernel")
token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
pattern = re.compile(
    r"^vespa-base-v([0-9]+)-"
    + re.escape(build_env)
    + "-"
    + re.escape(target)
    + "-"
    + re.escape(commit)
    + r"$"
)
versions = []
page = 1
while True:
    url = f"https://api.github.com/repos/{repo}/releases?per_page=100&page={page}"
    headers = {
        "Accept": "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
        "User-Agent": "searchkernel-native-base-resolver",
    }
    if token:
        headers["Authorization"] = f"Bearer {token}"
    request = urllib.request.Request(url, headers=headers)
    try:
        with urllib.request.urlopen(request) as response:
            releases = json.load(response)
    except urllib.error.HTTPError as exc:
        hint = "set GH_TOKEN/GITHUB_TOKEN or SK_NATIVE_BASE_ARTIFACT_URL"
        raise SystemExit(
            f"could not discover published native-base generations ({exc.code}); {hint}"
        ) from exc
    if not isinstance(releases, list):
        raise SystemExit("GitHub releases response was not a list")
    for release in releases:
        match = pattern.match(str(release.get("tag_name", "")))
        if match:
            versions.append(int(match.group(1)))
    if len(releases) < 100:
        break
    page += 1

if not versions:
    raise SystemExit(
        "no published native-base generation exists for "
        f"{build_env}/{target}/{commit}"
    )
print(max(versions))
