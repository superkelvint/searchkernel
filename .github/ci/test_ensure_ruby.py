#!/usr/bin/env python3
from __future__ import annotations

import os
import subprocess
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[2] / "tools" / "ci" / "runner-bootstrap" / "ensure-ruby.sh"


def fake_ruby(path: Path, usable: bool) -> None:
    path.write_text(
        "#!/usr/bin/env bash\n"
        "if [[ \"$*\" == *Gem::Version* ]]; then\n"
        f"  exit {0 if usable else 1}\n"
        "fi\n"
        "if [[ \"$1\" == \"--version\" ]]; then echo 'ruby fixture'; fi\n"
        "exit 0\n",
        encoding="utf-8",
    )
    path.chmod(0o755)


class EnsureRubyTests(unittest.TestCase):
    def test_old_system_ruby_reuses_cached_supported_ruby_without_installing(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            fakebin = root / "fakebin"
            cache = root / "cache"
            cached_bin = cache / "3.3.6" / "bin"
            fakebin.mkdir()
            cached_bin.mkdir(parents=True)

            fake_ruby(fakebin / "ruby", usable=False)
            (fakebin / "gem").write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            (fakebin / "gem").chmod(0o755)
            fake_ruby(cached_bin / "ruby", usable=True)
            (cached_bin / "gem").write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            (cached_bin / "gem").chmod(0o755)

            marker = root / "unexpected-install"
            for command in ("sudo", "curl"):
                tool = fakebin / command
                tool.write_text(
                    "#!/usr/bin/env bash\n"
                    f"touch {marker!s}\n"
                    "exit 99\n",
                    encoding="utf-8",
                )
                tool.chmod(0o755)

            github_path = root / "github-path"
            env = os.environ.copy()
            env["PATH"] = f"{fakebin}:/usr/bin:/bin"
            env["SEARCHKERNEL_RUBY_CACHE_ROOT"] = str(cache)
            env["GITHUB_PATH"] = str(github_path)

            result = subprocess.run(
                ["bash", str(SCRIPT)],
                env=env,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                check=False,
            )

            self.assertEqual(result.returncode, 0, result.stdout)
            self.assertFalse(marker.exists(), result.stdout)
            self.assertEqual(github_path.read_text(encoding="utf-8").strip(), str(cached_bin))

    def test_bootstrap_is_checksum_pinned_and_never_updates_apt_metadata(self) -> None:
        source = SCRIPT.read_text(encoding="utf-8")
        self.assertIn("8dc48fffaf270f86f1019053f28e51e4da4cce32a36760a0603a9aee67d7fd8d", source)
        self.assertIn("sha256sum --check --status", source)
        self.assertIn("sudo -n apt-get install -y --no-install-recommends", source)
        self.assertNotIn("apt-get update", source)


if __name__ == "__main__":
    unittest.main()
