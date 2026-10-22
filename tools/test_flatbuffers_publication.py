"""Exercise the real installer with controlled provisioning and real C++ probes."""
import hashlib
import os
from pathlib import Path
import re
import subprocess
import tarfile
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[1]


class PublicationTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="searchkernel-flatbuffers-publication-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.prefix = self.root / "published"
        self.fixture(self.prefix, "OLD")
        self.candidate = self.root / "next"
        self.fixture(self.candidate, "NEW")
        self.control = self.root / "control"
        self.control.mkdir()
        private = self.root / "repository"
        tools = private / "tools"
        tools.mkdir(parents=True)
        for source in (ROOT / "tools").iterdir():
            if source.name != "check-flatbuffers-cpp.sh":
                (tools / source.name).symlink_to(source)
        self.script(
            tools / "check-flatbuffers-cpp.sh",
            'if [[ "$1" == "$SEARCHKERNEL_TEST_PREFIX" ]]; then exit 1; fi\n'
            'exec bash "$SEARCHKERNEL_TEST_REAL_CHECKER" "$@"\n',
        )
        archive = self.root / "source.tar.gz"
        with tarfile.open(archive, "w:gz"):
            pass
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        text = (ROOT / "dev").read_text(encoding="utf-8")
        text, count = re.subn(
            r"(?m)^EXPECTED_FLATBUFFERS_SOURCE_SHA256=.*$",
            f"EXPECTED_FLATBUFFERS_SOURCE_SHA256={digest}", text,
        )
        self.assertEqual(count, 1)
        # Only the private fixture's source identity changes. Production's
        # pinned digest and authentication path remain untouched.
        self.dev = private / "dev"
        self.dev.write_text(text, encoding="utf-8")
        self.dev.chmod(0o755)
        fake = self.root / "fake-bin"
        fake.mkdir()
        self.script(fake / "curl", '''
while (($#)); do
    if [[ "$1" == --output ]]; then cp "$SEARCHKERNEL_TEST_ARCHIVE" "$2"; exit; fi
    shift
done
exit 2
''')
        self.script(fake / "cmake", '''
if [[ "$1" == --build ]]; then exit 0; fi
if [[ "$1" == --install ]]; then
    prefix=$(cat "$2/prefix")
    mkdir -p "$prefix/include"
    cp -a "$SEARCHKERNEL_TEST_TOOLCHAIN/include/." "$prefix/include/"
    if [[ -n "$SEARCHKERNEL_TEST_BARRIER" ]]; then
        touch "$SEARCHKERNEL_TEST_BARRIER/ready"
        while [[ ! -f "$SEARCHKERNEL_TEST_BARRIER/release" ]]; do sleep 0.01; done
    fi
    cp -a "$SEARCHKERNEL_TEST_TOOLCHAIN/." "$prefix/"
    exit 0
fi
build= prefix=
while (($#)); do
    case "$1" in
        -B) build=$2; shift 2 ;;
        -DCMAKE_INSTALL_PREFIX=*) prefix=${1#*=}; shift ;;
        *) shift ;;
    esac
done
mkdir -p "$build"
printf '%s\n' "$prefix" > "$build/prefix"
''')
        self.environment = os.environ | {
            "PATH": str(fake) + os.pathsep + os.environ["PATH"],
            "FLATBUFFERS_INSTALL_PREFIX": str(self.prefix),
            "SEARCHKERNEL_TEST_PREFIX": str(self.prefix),
            "SEARCHKERNEL_TEST_REAL_CHECKER": str(ROOT / "tools/check-flatbuffers-cpp.sh"),
            "SEARCHKERNEL_TEST_ARCHIVE": str(archive),
            "SEARCHKERNEL_TEST_TOOLCHAIN": str(self.candidate),
            "SEARCHKERNEL_TEST_BARRIER": "",
        }

    def script(self, path, body):
        path.write_text("#!/usr/bin/env bash\nset -euo pipefail\n" + body, encoding="utf-8")
        path.chmod(0o755)

    def fixture(self, prefix, identity):
        for directory in ("bin", "include/flatbuffers", "lib"):
            (prefix / directory).mkdir(parents=True, exist_ok=True)
        self.script(prefix / "bin/flatc", 'echo "flatc version 25.12.19"\n')
        (prefix / "include/flatbuffers/base.h").write_text(
            "#define FLATBUFFERS_VERSION_MAJOR 25\n"
            "#define FLATBUFFERS_VERSION_MINOR 12\n"
            f"#define FLATBUFFERS_VERSION_REVISION 19\n// {identity}\n", encoding="utf-8",
        )
        (prefix / "include/flatbuffers/idl.h").write_text(
            "namespace flatbuffers { class Parser { public: Parser(); bool Parse(const char*); }; }\n",
            encoding="utf-8",
        )
        source = prefix / "parser.cc"
        source.write_text(
            "#include <flatbuffers/idl.h>\nflatbuffers::Parser::Parser() {}\n"
            "bool flatbuffers::Parser::Parse(const char*) { return true; }\n", encoding="utf-8",
        )
        subprocess.run(
            [os.environ.get("CXX", "c++"), "-I" + str(prefix / "include"), "-c", str(source), "-o", str(prefix / "parser.o")],
            check=True,
        )
        subprocess.run(
            [os.environ.get("AR", "ar"), "rcs", str(prefix / "lib/libflatbuffers.a"), str(prefix / "parser.o")],
            check=True,
        )

    def snapshot(self):
        return {str(path.relative_to(self.prefix)): path.read_bytes()
                for path in self.prefix.rglob("*") if path.is_file()}

    def resolve(self):
        environment = self.environment | {"FLATBUFFERS_CPP_ROOT": str(self.prefix)}
        environment.pop("FLATC_PATH", None)
        result = subprocess.run(
            ["bash", str(ROOT / "tools/resolve-flatbuffers.sh"), "root"],
            env=environment, capture_output=True, text=True, check=True,
        )
        return Path(result.stdout.strip())

    def test_failed_candidate_does_not_modify_previous_installation(self):
        before = self.snapshot()
        (self.candidate / "lib/libflatbuffers.a").write_bytes(b"invalid archive")
        result = subprocess.run(
            [str(self.dev), "install", "flatbuffers"], env=self.environment,
            capture_output=True, text=True, timeout=45,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("compile/link probe failed", result.stdout + result.stderr)
        self.assertEqual(self.snapshot(), before)

    def test_reader_remains_pinned_through_partial_install_and_publication(self):
        before = self.snapshot()
        pinned = self.resolve()
        process = subprocess.Popen(
            [str(self.dev), "install", "flatbuffers"],
            env=self.environment | {"SEARCHKERNEL_TEST_BARRIER": str(self.control)},
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
        )
        try:
            deadline = time.monotonic() + 30
            while not (self.control / "ready").exists():
                if process.poll() is not None or time.monotonic() > deadline:
                    self.fail("installer did not reach the controlled partial-install barrier")
                time.sleep(0.01)
            self.assertEqual(self.snapshot(), before)
            self.assertEqual(self.resolve(), pinned)
        finally:
            (self.control / "release").touch()
            try:
                output, _ = process.communicate(timeout=45)
            except subprocess.TimeoutExpired:
                process.kill()
                process.communicate()
                raise
        self.assertEqual(process.returncode, 0, output)
        current = self.resolve()
        self.assertNotEqual(current, pinned)
        self.assertEqual((pinned / "include/flatbuffers/base.h").read_bytes(), before["include/flatbuffers/base.h"])
        self.assertIn("NEW", (current / "include/flatbuffers/base.h").read_text())
        self.assertTrue((current / "lib/libflatbuffers.a").is_file())


if __name__ == "__main__":
    unittest.main()
