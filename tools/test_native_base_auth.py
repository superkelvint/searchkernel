#!/usr/bin/env python3
import importlib.util
import io
import json
import os
import pathlib
import stat
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

ROOT = pathlib.Path(__file__).resolve().parent.parent
SELECTOR = ROOT / "tools" / "native-base-release-selection.py"
INSTALLER = ROOT / "tools" / "install-native-base-artifact.sh"
RESOLVER = ROOT / "tools" / "resolve-native-sdk.sh"


def load_selector():
    spec = importlib.util.spec_from_file_location("native_base_release_selection", SELECTOR)
    if spec is None or spec.loader is None:
        raise RuntimeError("could not load native-base release selector")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


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

    def release_payload(self):
        return [{
            "draft": False,
            "prerelease": False,
            "published_at": "2026-09-26T00:00:00Z",
            "tag_name": "fixture",
            "assets": [],
        }]

    def test_release_discovery_uses_public_https_without_token(self):
        selector = load_selector()
        payload = self.release_payload()
        requests = []

        def fake_urlopen(request):
            requests.append(request)
            return io.BytesIO(json.dumps(payload).encode("utf-8"))

        with mock.patch.dict(os.environ, {"GH_TOKEN": "", "GITHUB_TOKEN": ""}):
            with mock.patch.object(selector.urllib.request, "urlopen", side_effect=fake_urlopen):
                self.assertEqual(selector.fetch_releases("superkelvint/searchkernel"), payload)

        self.assertEqual(len(requests), 1)
        self.assertIsNone(requests[0].get_header("Authorization"))
        self.assertIn(
            "https://api.github.com/repos/superkelvint/searchkernel/releases",
            requests[0].full_url,
        )

    def test_release_discovery_uses_authenticated_https_for_private_repo(self):
        selector = load_selector()
        payload = self.release_payload()
        requests = []

        def fake_urlopen(request):
            requests.append(request)
            return io.BytesIO(json.dumps(payload).encode("utf-8"))

        with mock.patch.dict(
            os.environ,
            {"GH_TOKEN": "fixture-token", "GITHUB_TOKEN": ""},
        ):
            with mock.patch.object(selector.urllib.request, "urlopen", side_effect=fake_urlopen):
                self.assertEqual(selector.fetch_releases("superkelvint/searchkernel"), payload)

        self.assertEqual(len(requests), 1)
        self.assertEqual(requests[0].get_header("Authorization"), "Bearer fixture-token")

    def test_installer_uses_https_without_gh_for_public_release(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = pathlib.Path(tmp)
            fakebin = tmp / "bin"
            fakebin.mkdir()
            curl_log = tmp / "curl.log"
            gh_log = tmp / "gh.log"
            curl = fakebin / "curl"
            gh = fakebin / "gh"
            self.write_executable(
                curl,
                "#!/usr/bin/env bash\n"
                "printf '%s\\n' \"$*\" >> \"$TEST_CURL_LOG\"\n"
                "exit 22\n",
            )
            self.write_executable(
                gh,
                "#!/usr/bin/env bash\n"
                "printf '%s\\n' \"$*\" >> \"$TEST_GH_LOG\"\n"
                "exit 1\n",
            )
            env = self.clean_env()
            env["PATH"] = str(fakebin) + os.pathsep + env.get("PATH", "")
            env["TEST_CURL_LOG"] = str(curl_log)
            env["TEST_GH_LOG"] = str(gh_log)
            url = (
                "https://github.com/superkelvint/searchkernel/releases/download/"
                "vespa-base-v7-host-x86_64-unknown-linux-gnu-fixture-vespa-commit/"
                "searchkernel-vespa-base-v7-host-x86_64-unknown-linux-gnu-fixture-vespa-commit.tar.gz"
            )
            result = subprocess.run(
                ["bash", str(INSTALLER), url, str(tmp / "cache"), "host",
                 "x86_64-unknown-linux-gnu", "fixture-vespa-commit"],
                env=env,
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertEqual(result.returncode, 10)
            self.assertIn(url, curl_log.read_text(encoding="utf-8"))
            self.assertFalse(gh_log.exists(), "consumer download unexpectedly invoked gh")
            self.assertIn("published Vespa base is unavailable", result.stderr)

    def test_resolver_only_constructs_release_url_after_generation_resolution(self):
        source = RESOLVER.read_text(encoding="utf-8")
        block = source.split("resolve_published_base_identity() {", 1)[1].split(
            "\n}\n\ninstalled_published_generation", 1
        )[0]
        discovery = block.index("generation=$(latest_published_generation)")
        failure = block.index("return 1", discovery)
        tag = block.index('tag="vespa-base-v${generation}', discovery)
        self.assertLess(discovery, failure)
        self.assertLess(failure, tag)


if __name__ == "__main__":
    unittest.main()
