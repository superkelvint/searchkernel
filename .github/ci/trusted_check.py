#!/usr/bin/env python3
"""Publish a candidate-head check through the dedicated trusted CI GitHub App."""
from __future__ import annotations

import argparse
import json
import os
import urllib.error
import urllib.parse
import urllib.request

API_VERSION = "2022-11-28"


def request(token: str, method: str, url: str, payload: dict | None = None) -> dict:
    data = None if payload is None else json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(url, data=data, method=method)
    req.add_header("Accept", "application/vnd.github+json")
    req.add_header("Authorization", f"Bearer {token}")
    req.add_header("X-GitHub-Api-Version", API_VERSION)
    if data is not None:
        req.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(req, timeout=20) as response:
            body = response.read()
    except urllib.error.HTTPError as exc:
        detail = exc.read().decode("utf-8", errors="replace")
        raise SystemExit(f"GitHub API {method} {url} failed: {exc.code}: {detail}") from exc
    return json.loads(body) if body else {}


def publish(
    *, token: str, api_url: str, repo: str, sha: str, name: str, external_id: str,
    app_id: int, conclusion: str, summary: str, details_url: str | None
) -> dict:
    query = urllib.parse.urlencode({"check_name": name, "filter": "all", "per_page": 100})
    listing = request(token, "GET", f"{api_url}/repos/{repo}/commits/{sha}/check-runs?{query}")
    existing = next(
        (
            check for check in listing.get("check_runs", [])
            if check.get("external_id") == external_id
            and int(check.get("app", {}).get("id", -1)) == app_id
        ),
        None,
    )
    payload = {
        "name": name,
        "head_sha": sha,
        "external_id": external_id,
        "status": "completed",
        "conclusion": conclusion,
        "output": {"title": name, "summary": summary[:65535]},
    }
    if details_url:
        payload["details_url"] = details_url
    if existing is None:
        return request(token, "POST", f"{api_url}/repos/{repo}/check-runs", payload)
    payload.pop("head_sha", None)
    return request(token, "PATCH", f"{api_url}/repos/{repo}/check-runs/{existing['id']}", payload)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", required=True)
    parser.add_argument("--sha", required=True)
    parser.add_argument("--name", required=True)
    parser.add_argument("--external-id", required=True)
    parser.add_argument("--app-id", required=True, type=int)
    parser.add_argument("--conclusion", required=True, choices=("success", "failure", "neutral", "cancelled", "timed_out", "action_required"))
    parser.add_argument("--summary", required=True)
    parser.add_argument("--details-url")
    args = parser.parse_args()
    token = os.environ.get("TRUSTED_CI_APP_TOKEN", "")
    if not token:
        raise SystemExit("TRUSTED_CI_APP_TOKEN is required")
    api_url = os.environ.get("GITHUB_API_URL", "https://api.github.com").rstrip("/")
    result = publish(
        token=token,
        api_url=api_url,
        repo=args.repo,
        sha=args.sha,
        name=args.name,
        external_id=args.external_id,
        app_id=args.app_id,
        conclusion=args.conclusion,
        summary=args.summary,
        details_url=args.details_url,
    )
    print(f"published trusted check {result.get('id', '<unknown>')} on {args.sha}: {args.conclusion}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
