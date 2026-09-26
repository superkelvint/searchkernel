#!/usr/bin/env python3
import json
import os
import pathlib
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent
SELECTOR = ROOT / "tools" / "native-base-release-selection.py"
INSTALLER = ROOT / "tools" / "install-native-base-artifact.sh"
RESOLVER = ROOT / "tools" / "resolve-native-sdk.sh"


class NativeBaseAuthTest(unittest.TestCase):
    def clean_env(self):
        env = os.environ.copy()
        for name in (
            "GH_TOKEN",
            "GITHUB_TOKEN",
            "SK_NATIVE_SDK",
            "SK_NATIVE_BASE_ARTIFACT_URL",
            "SK_NATIVE_BASE_ARTIFACT_GENERATION",
            "SK_NATIVE_DISABLE_PUBLISHED_BASE",
            "SK_NATIVE_CACHE_DIR",
            "VESPA_ROOT",
            "VESPA_LIBRARY_ROOT",
        ):
            env.pop(name, None)
        return env

    def write_executable(self, path, text):
        path.write_text(text, encoding="utf-8")
        path.chmod(path.stat().st_mode | stat.S_IXUSR)

    def test_release_selection_uses_authenticated_gh_without_exported_token(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = pathlib.Path(tmp)
            fakebin = tmp / "bin"
            fakebin.mkdir()
            log = tmp / "gh.log"
            tag = "vespa-base-v7-host-x86_64-unknown-linux-gnu-fixture-vespa-commit"
            asset = "searchkernel-vespa-base-v7-host-x86_64-unknown-linux-gnu-fixture-vespa-commit.tar.gz"
            payload = [{
                "draft": False,
                "prerelease": False,
                "published_at": "2026-09-26T00:00:00Z",
                "tag_name": tag,
                "assets": [
                    {"state": "uploaded", "size": 1, "name": asset},
                    {"state": "uploaded", "size": 1, "name": asset + ".sha256"},
                ],
            }]
            gh = fakebin / "gh"
            self.write_executable(
                gh,
                "#!/usr/bin/env python3\n"
                "import os, pathlib, sys\n"
                "pathlib.Path(os.environ['TEST_GH_LOG']).write_text(' '.join(sys.argv[1:]) + '\\n', encoding='utf-8')\n"
                "print(os.environ['TEST_GH_PAYLOAD'])\n",
            )
            env = self.clean_env()
            env["PATH"] = str(fakebin) + os.pathsep + env.get("PATH", "")
            env["TEST_GH_LOG"] = str(log)
            env["TEST_GH_PAYLOAD"] = json.dumps(payload)
            result = subprocess.run(
                [sys.executable, str(SELECTOR), "host", "x86_64-unknown-linux-gnu", "fixture-vespa-commit"],
                env=env,
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout.strip(), "7")
            self.assertIn("api repos/", log.read_text(encoding="utf-8"))

    def test_installer_tries_gh_release_download_without_exported_token(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = pathlib.Path(tmp)
            fakebin = tmp / "bin"
            fakebin.mkdir()
            log = tmp / "gh.log"
            gh = fakebin / "gh"
            self.write_executable(
                gh,
                "#!/usr/bin/env bash\n"
                "printf '%s\\n' \"$*\" >> \"$TEST_GH_LOG\"\n"
                "exit 1\n",
            )
            env = self.clean_env()
            env["PATH"] = str(fakebin) + os.pathsep + env.get("PATH", "")
            env["TEST_GH_LOG"] = str(log)
            url = (
                "https://github.com/superkelvint/searchkernel/releases/download/"
                "vespa-base-v7-host-x86_64-unknown-linux-gnu-fixture-vespa-commit/"
                "searchkernel-vespa-base-v7-host-x86_64-unknown-linux-gnu-fixture-vespa-commit.tar.gz"
            )
            result = subprocess.run(
                ["bash", str(INSTALLER), url, str(tmp / "cache"), "host", "x86_64-unknown-linux-gnu", "fixture-vespa-commit"],
                env=env,
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertEqual(result.returncode, 10)
            self.assertIn("release download", log.read_text(encoding="utf-8"))
            self.assertIn("published Vespa base is unavailable", result.stderr)

    def test_generation_discovery_failure_never_constructs_v_host_url(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = pathlib.Path(tmp)
            repo = tmp / "repo"
            tools = repo / "tools"
            manifest_dir = repo / "core" / "native" / "vespa"
            tools.mkdir(parents=True)
            manifest_dir.mkdir(parents=True)
            for name in (
                "resolve-native-sdk.sh",
                "resolve-python.sh",
                "native-base-release-selection.py",
                "install-native-base-artifact.sh",
            ):
                shutil.copy2(ROOT / "tools" / name, tools / name)
            (manifest_dir / "manifest.json").write_text(
                json.dumps({"vespa_commit": "fixture-vespa-commit"}) + "\n",
                encoding="utf-8",
            )
            subprocess.run(["git", "init", "-q"], cwd=repo, check=True)

            fakebin = tmp / "bin"
            fakebin.mkdir()
            gh = fakebin / "gh"
            self.write_executable(
                gh,
                "#!/usr/bin/env bash\n"
                "echo 'fixture gh discovery failure' >&2\n"
                "exit 1\n",
            )
            home = tmp / "home"
            home.mkdir()
            env = self.clean_env()
            env["PATH"] = str(fakebin) + os.pathsep + env.get("PATH", "")
            env["HOME"] = str(home)
            env["XDG_CACHE_HOME"] = str(tmp / "cache")
            env["PYTHON"] = sys.executable
            env["SEARCHKERNEL_BUILD_ENV"] = "host"
            env["SK_NATIVE_TARGET"] = "x86_64-unknown-linux-gnu"
            env["SK_NATIVE_CACHE_BUILDER"] = "/bin/false"
            result = subprocess.run(
                ["bash", str(tools / "resolve-native-sdk.sh"), "ensure"],
                cwd=repo,
                env=env,
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("could not discover published native-base generations with authenticated gh", result.stderr)
            self.assertNotIn("vespa-base-v-host-", result.stderr)


if __name__ == "__main__":
    unittest.main()
