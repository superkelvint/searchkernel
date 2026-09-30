import json
import re
from pathlib import Path

from setuptools import setup
from setuptools.command.bdist_wheel import bdist_wheel as _bdist_wheel


ROOT = Path(__file__).resolve().parents[2]
RELEASE_SUPPORT = json.loads((ROOT / "docs" / "release-support.json").read_text())
PYTHON_WHEEL_PLATFORM_TAG = RELEASE_SUPPORT.get("python_wheel_platform_tag")
if not isinstance(PYTHON_WHEEL_PLATFORM_TAG, str) or not re.fullmatch(
    r"manylinux_[0-9]+_[0-9]+_x86_64", PYTHON_WHEEL_PLATFORM_TAG
):
    raise RuntimeError(
        "docs/release-support.json must declare a valid x86_64 manylinux "
        "python_wheel_platform_tag"
    )


class SearchKernelWheel(_bdist_wheel):
    def finalize_options(self) -> None:
        super().finalize_options()
        self.root_is_pure = False

    def get_tag(self) -> tuple[str, str, str]:
        return ("py3", "none", PYTHON_WHEEL_PLATFORM_TAG)


setup(cmdclass={"bdist_wheel": SearchKernelWheel})
