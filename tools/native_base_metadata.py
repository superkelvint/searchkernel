"""Shared normalization rules for producer-root paths in CMake metadata."""

from __future__ import annotations

import re


def normalize_absolute_root(text: str, source: str, destination: str) -> str:
    source = source.rstrip("/")
    if not source:
        return text
    for prefix in ("-I", "-L", "-isystem"):
        text = text.replace(prefix + source + "/", prefix + destination.rstrip("/") + "/")
    pattern = re.compile(r"(?<![A-Za-z0-9_./-])" + re.escape(source) + r"(?=/|$)")
    return pattern.sub(destination, text)
