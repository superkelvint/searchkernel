"""Lazy submodule imports for generated FlatBuffers packages."""

import importlib

def __getattr__(name):
    return importlib.import_module(f"{__name__}.{name}")
