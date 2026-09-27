#!/usr/bin/env python3
"""Compatibility entry point for the canonical Cargo native adapter resolver."""

from pathlib import Path
import runpy

SCRIPT = Path(__file__).resolve().parents[2] / "tools" / "resolve-cargo-native-adapter-lib.py"
runpy.run_path(str(SCRIPT), run_name="__main__")
