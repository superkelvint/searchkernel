"""Runtime support for recursive generated FlatBuffers modules."""

from __future__ import annotations

import importlib


class _LazyReference:
    def __init__(self, module_name, attribute=None):
        self._module_name = module_name
        self._attribute = attribute

    def _resolve(self):
        value = importlib.import_module(self._module_name)
        return getattr(value, self._attribute) if self._attribute else value

    def __call__(self, *args, **kwargs):
        return self._resolve()(*args, **kwargs)

    def __getattr__(self, name):
        return getattr(self._resolve(), name)

def lazy_type(module_name, attribute):
    return _LazyReference(module_name, attribute)

def lazy_module(module_name):
    return _LazyReference(module_name)
