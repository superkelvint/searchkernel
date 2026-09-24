#!/usr/bin/env python3
import importlib.util
import sys
from pathlib import Path

module_path = Path(sys.argv[1])
fixture = Path(sys.argv[2])
(fixture / "core/native/vespa").mkdir(parents=True)
(fixture / "tools").mkdir(parents=True)
(fixture / "core/native/vespa/build-sdk.sh").write_text("builder-a\n", encoding="utf-8")
(fixture / "tools/copy-shared-library-into-sdk.sh").write_text("packager-a\n", encoding="utf-8")

spec = importlib.util.spec_from_file_location("native_build_fingerprint", module_path)
module = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(module)

before = module.packaging_digest(fixture)
(fixture / "tools/copy-shared-library-into-sdk.sh").write_text("packager-b\n", encoding="utf-8")
after = module.packaging_digest(fixture)
assert before != after
