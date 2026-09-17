"""Setuptools hook for assembling the optional platform-native artifact."""

from __future__ import annotations

import os
import platform
import shutil
from pathlib import Path

from setuptools import setup
from setuptools.command.build_py import build_py as _build_py
from setuptools.dist import Distribution


class BinaryDistribution(Distribution):
    """Tell wheel that package data includes a platform-native binary."""

    def has_ext_modules(self):
        return True


class build_py(_build_py):
    """Copy the explicitly selected native library into the wheel."""

    def run(self):
        super().run()

        # Flat-layout setuptools builds can retain files from an older local
        # build directory. Tests are not package modules and must never leak
        # into a wheel, including when the build directory is reused.
        tests = Path(self.build_lib) / "tests"
        if tests.is_dir():
            shutil.rmtree(tests)

        source_name = os.environ.get("SEARCHKERNEL_NATIVE_LIBRARY")
        if not source_name:
            return

        source = Path(source_name)
        if not source.is_file():
            raise RuntimeError(
                "SEARCHKERNEL_NATIVE_LIBRARY does not name a file: "
                f"{source}"
            )

        platform_name = {
            ("Linux", "x86_64"): "linux-x86_64",
            ("Linux", "amd64"): "linux-x86_64",
            ("Darwin", "x86_64"): "darwin-x86_64",
            ("Darwin", "arm64"): "darwin-arm64",
        }.get((platform.system(), platform.machine().lower()))
        if platform_name is None:
            raise RuntimeError(
                "no Search Kernel native package target for "
                f"{platform.system()}-{platform.machine()}"
            )

        target = (
            Path(self.build_lib)
            / "searchkernel"
            / "native"
            / platform_name
            / source.name
        )
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)


setup(distclass=BinaryDistribution, cmdclass={"build_py": build_py})
