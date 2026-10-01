"""The retained Python package must import without retired client exports."""

import sys
import unittest
import importlib.util
import ast
import os
import subprocess
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parents[1]))


class PackageImportTest(unittest.TestCase):
    def test_all_public_exports_resolve(self):
        import searchkernel

        self.assertTrue(searchkernel.__all__)
        for name in searchkernel.__all__:
            with self.subTest(name=name):
                self.assertTrue(hasattr(searchkernel, name))

    def test_generated_bindings_stay_inside_searchkernel_package(self):
        import searchkernel

        generated = Path(searchkernel.__file__).parent / "_wire" / "v1" / "generated"
        self.assertNotIn(str(generated), sys.path)
        self.assertIsNone(importlib.util.find_spec("SearchKernel"))

    def test_installed_package_does_not_publish_generated_top_level_package(self):
        package_root = Path(__file__).parents[1]
        with tempfile.TemporaryDirectory() as temporary:
            target = Path(temporary) / "site"
            subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "pip",
                    "install",
                    "--no-deps",
                    "--target",
                    str(target),
                    str(package_root),
                ],
                check=True,
                stdout=subprocess.DEVNULL,
            )
            environment = os.environ.copy()
            environment["PYTHONPATH"] = str(target)
            subprocess.run(
                [
                    sys.executable,
                    "-c",
                    "import importlib.util, searchkernel; "
                    "assert importlib.util.find_spec('SearchKernel') is None; "
                    "assert all('SearchKernel' not in path for path in __import__('sys').path)",
                ],
                cwd=temporary,
                env=environment,
                check=True,
            )

    def test_public_python_modules_do_not_import_generated_bindings(self):
        package = Path(__file__).parents[1] / "searchkernel"
        for name in ("client.py", "ergonomic.py"):
            with self.subTest(name=name):
                tree = ast.parse((package / name).read_text(encoding="utf-8"))
                imports = [
                    node.module
                    for node in ast.walk(tree)
                    if isinstance(node, ast.ImportFrom) and node.module
                ]
                imports += [
                    alias.name
                    for node in ast.walk(tree)
                    if isinstance(node, ast.Import)
                    for alias in node.names
                ]
                self.assertFalse(
                    any(".generated." in module or "_wire_generated" in module for module in imports),
                    f"{name} imports generated wire bindings directly",
                )

    def test_ergonomic_builder_returns_searchkernel_semantic_model(self):
        from searchkernel._semantic import SemanticTable
        from searchkernel.ergonomic import Text

        self.assertIsInstance(Text().definition("title"), SemanticTable)

    def test_request_codec_writes_first_wire_major(self):
        from searchkernel._semantic import DescribeRequestT
        from searchkernel._wire import PROTOCOL_VERSION, pack_request
        from searchkernel._wire.v1.generated.SearchKernel.Protocol import RequestEnvelope

        request = pack_request("describe", DescribeRequestT())
        envelope = RequestEnvelope.RequestEnvelope.GetRootAs(request, 0)
        self.assertEqual(PROTOCOL_VERSION, 1)
        self.assertEqual(envelope.ProtocolVersion(), 1)

    def test_checked_in_envelopes_use_first_wire_major(self):
        from searchkernel._wire.v1.generated.SearchKernel.Protocol.RequestEnvelope import RequestEnvelope
        from searchkernel._wire.v1.generated.SearchKernel.Protocol.ResponseEnvelope import ResponseEnvelope

        repository = Path(__file__).parents[3]
        fixture_root = repository / "core/crates/searchkernel-dispatch/tests/fixtures/portable"
        for extension, envelope_type, getter in (
            ("*.skrq", RequestEnvelope, "GetRootAs"),
            ("*.skrs", ResponseEnvelope, "GetRootAsResponseEnvelope"),
        ):
            for fixture in fixture_root.rglob(extension):
                with self.subTest(fixture=fixture.relative_to(repository)):
                    root = getattr(envelope_type, getter)(fixture.read_bytes(), 0)
                    self.assertEqual(root.ProtocolVersion(), 1)


if __name__ == "__main__":
    unittest.main()
