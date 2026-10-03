#!/usr/bin/env python3
"""Select the newest complete published SearchKernel native-base generation."""

import argparse
import json
import os
import re
import sys
import urllib.error
import urllib.request


def release_generation(release, build_env, target, commit):
    if release.get("draft") is True:
        return None
    if release.get("prerelease") is True:
        return None
    if not release.get("published_at"):
        return None

    pattern = re.compile(
        r"^vespa-base-v([0-9]+)-"
        + re.escape(build_env)
        + "-"
        + re.escape(target)
        + "-"
        + re.escape(commit)
        + r"$"
    )
    match = pattern.match(str(release.get("tag_name", "")))
    if match is None:
        return None

    generation = int(match.group(1))
    asset = "searchkernel-vespa-base-v{}-{}-{}-{}.tar.gz".format(
        generation, build_env, target, commit
    )
    checksum = asset + ".sha256"

    uploaded = set()
    for candidate in release.get("assets", []):
        if candidate.get("state") != "uploaded":
            continue
        size = candidate.get("size")
        if not isinstance(size, int) or size <= 0:
            continue
        uploaded.add(str(candidate.get("name")))

    if asset not in uploaded or checksum not in uploaded:
        return None
    return generation


def select_generation(releases, build_env, target, commit):
    generations = []
    for release in releases:
        generation = release_generation(release, build_env, target, commit)
        if generation is not None:
            generations.append(generation)
    if not generations:
        raise SystemExit(
            "no complete published native-base generation exists for "
            "{}/{}/{}".format(build_env, target, commit)
        )
    return max(generations)


def fetch_releases(repo):
    token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    releases = []
    page = 1
    while True:
        url = "https://api.github.com/repos/{}/releases?per_page=100&page={}".format(
            repo, page
        )
        headers = {
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
            "User-Agent": "searchkernel-native-base-selector",
        }
        if token:
            headers["Authorization"] = "Bearer " + token
        request = urllib.request.Request(url, headers=headers)
        try:
            with urllib.request.urlopen(request) as response:
                batch = json.load(response)
        except urllib.error.HTTPError as exc:
            hint = (
                "authenticate with gh ('gh auth login') or set "
                "GH_TOKEN/GITHUB_TOKEN when the repository is private"
            )
            raise SystemExit(
                "could not discover published native-base generations "
                "({}); {}".format(exc.code, hint)
            )
        if not isinstance(batch, list):
            raise SystemExit("GitHub releases response was not a list")
        releases.extend(batch)
        if len(batch) < 100:
            break
        page += 1
    return releases
def read_releases_file(path):
    with open(path, encoding="utf-8") as stream:
        data = json.load(stream)
    if not isinstance(data, list):
        raise SystemExit("{}: expected a JSON release list".format(path))
    if not all(isinstance(item, dict) for item in data):
        raise SystemExit("{}: every release entry must be an object".format(path))
    return data


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("build_env", choices=("ci", "host"))
    parser.add_argument("target")
    parser.add_argument("commit")
    parser.add_argument(
        "--repo",
        default=os.environ.get(
            "SEARCHKERNEL_GITHUB_REPO", "superkelvint/searchkernel"
        ),
    )
    parser.add_argument("--releases-file")
    args = parser.parse_args()

    if args.releases_file:
        releases = read_releases_file(args.releases_file)
    else:
        releases = fetch_releases(args.repo)

    print(select_generation(releases, args.build_env, args.target, args.commit))
    return 0


if __name__ == "__main__":
    sys.exit(main())
