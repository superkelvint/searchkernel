#!/usr/bin/env python3
"""Build a deterministic high-coverage KStem corpus from the pinned Vespa source.

The corpus is derived from the oracle source, not from the Rust implementation.
It includes every lowercase ASCII string literal in Vespa's kstem package plus
systematic suffix mutations that exercise KStem's transformation routines.
"""
from __future__ import annotations

import argparse
import pathlib
import re

LOWER_STRING = re.compile(r'"([a-z]+)"')
ASCII_LOWER = re.compile(r'^[a-z]+$')

# Mirrors the suffix families exercised by KStemmer, without encoding expected
# behavior. Blindly combining them with the pinned dictionary gives broad branch
# coverage while leaving Vespa as the sole semantic oracle.
SUFFIXES = (
    "s", "es", "ies",
    "ed", "ied", "ing",
    "ity", "ness", "ion",
    "er", "or", "ly", "al", "ive", "ize", "ment",
    "able", "ible", "ism", "ic", "ncy", "nce",
)

CURATED = {
    # Algorithm boundaries / non-stemmable inputs.
    "a", "ab", "abc", "z" * 49, "z" * 50,
    "Running", "ABC", "abc123", "foo-bar", "café",
    # Known examples that make failures easy to recognize.
    "running", "runners", "foxes", "jumped", "generously", "relational",
    "aging", "going", "goes", "lying", "using", "dying", "tying",
    "aeronautical", "mathematical", "political", "metaphysical",
    "ambiguity", "generosity", "miscarriage",
}


def find_source(vespa: pathlib.Path) -> pathlib.Path:
    source = vespa / "linguistics/src/main/java/com/yahoo/language/simple/kstem"
    if not (source / "KStemmer.java").is_file():
        raise SystemExit(f"not a Vespa kstem source tree: {source}")
    return source


def source_seeds(source: pathlib.Path) -> set[str]:
    seeds: set[str] = set()
    for java in sorted(source.glob("*.java")):
        text = java.read_text(encoding="utf-8")
        seeds.update(LOWER_STRING.findall(text))
    return seeds


def add_if_safe(corpus: set[str], term: str) -> None:
    # Line-oriented protocol: no tabs/newlines. Non-ASCII curated cases are OK.
    if "\t" not in term and "\n" not in term and "\r" not in term:
        corpus.add(term)


def build(seeds: set[str]) -> list[str]:
    corpus: set[str] = set(CURATED)

    for base in sorted(seeds):
        add_if_safe(corpus, base)
        if not ASCII_LOWER.fullmatch(base):
            continue

        # Direct suffix pressure. Keep generated algorithmic inputs under
        # KStemmer's 50-char active limit; explicit 50-char boundary is curated.
        for suffix in SUFFIXES:
            candidate = base + suffix
            if len(candidate) < 50:
                add_if_safe(corpus, candidate)

        # Common orthographic transitions used by KStem branches.
        if base.endswith("y") and len(base) > 1:
            for candidate in (base[:-1] + "ies", base[:-1] + "ied"):
                if len(candidate) < 50:
                    add_if_safe(corpus, candidate)

        if base.endswith("e") and len(base) > 1:
            for candidate in (base[:-1] + "ing", base + "d"):
                if len(candidate) < 50:
                    add_if_safe(corpus, candidate)

        if base.endswith("ion") and len(base) > 3:
            candidate = base[:-3] + "ive"
            if len(candidate) < 50:
                add_if_safe(corpus, candidate)

        if base.endswith("ize") and len(base) > 3:
            candidate = base[:-3] + "ization"
            if len(candidate) < 50:
                add_if_safe(corpus, candidate)

        # Exercise doubled-consonant fallbacks for suitable ASCII bases.
        if len(base) >= 2 and base[-1] not in "aeiouy":
            for suffix in ("ed", "ing", "er"):
                candidate = base + base[-1] + suffix
                if len(candidate) < 50:
                    add_if_safe(corpus, candidate)

    return sorted(corpus)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--vespa-src", required=True, type=pathlib.Path)
    parser.add_argument("--out", required=True, type=pathlib.Path)
    args = parser.parse_args()

    source = find_source(args.vespa_src)
    seeds = source_seeds(source)
    corpus = build(seeds)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text("".join(term + "\n" for term in corpus), encoding="utf-8")
    print(f"kstem corpus: {len(seeds)} pinned-source seeds -> {len(corpus)} cases")


if __name__ == "__main__":
    main()
