use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn freshly_generated_python_models_are_keyword_only() {
    let repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let bfbs_root = repository_root.join("core/crates/searchkernel-json/bfbs");
    let output_root = std::env::temp_dir().join(format!(
        "searchkernel-bindgen-keyword-only-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is before Unix epoch")
            .as_nanos()
    ));
    fs::create_dir_all(&output_root).expect("create generator output directory");

    let generation = Command::new(env!("CARGO_BIN_EXE_searchkernel-bindgen"))
        .args([
            "generate",
            "--bfbs-root",
            bfbs_root.to_str().expect("BFBS path is UTF-8"),
            "--output-root",
            output_root.to_str().expect("output path is UTF-8"),
        ])
        .output()
        .expect("run searchkernel-bindgen");
    assert!(
        generation.status.success(),
        "generator failed:\n{}\n{}",
        String::from_utf8_lossy(&generation.stdout),
        String::from_utf8_lossy(&generation.stderr)
    );

    let generated_models = output_root.join("clients/python/searchkernel/generated/models.py");
    let python = r#"
import dataclasses
import importlib.util
import inspect
import pathlib
import sys
import types

path = sys.argv[1]
repository_root = pathlib.Path(sys.argv[2])
searchkernel = types.ModuleType("searchkernel")
searchkernel.__path__ = [str(repository_root / "clients/python/searchkernel")]
sys.modules["searchkernel"] = searchkernel
generated_package = types.ModuleType("searchkernel.generated")
generated_package.__path__ = [str(pathlib.Path(path).parent)]
sys.modules["searchkernel.generated"] = generated_package
spec = importlib.util.spec_from_file_location("searchkernel.generated.models", path)
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)
models = {
    value for value in vars(module).values()
    if isinstance(value, type)
    and value.__module__ == module.__name__
    and dataclasses.is_dataclass(value)
}
assert len(models) >= 4, len(models)
for model in models:
    assert all(
        parameter.kind is inspect.Parameter.KEYWORD_ONLY
        for parameter in inspect.signature(model).parameters.values()
    ), (model.__name__, inspect.signature(model))

document = module.Document(id="1", fields={})
for model, args in (
    (module.Document, ("1", {})),
    (module.UpsertRequest, (document,)),
    (module.SearchRequest, ("query",)),
    (module.IndexDefinition, ("products", [])),
):
    try:
        model(*args)
    except TypeError:
        pass
    else:
        raise AssertionError(f"{model.__name__} accepted positional arguments")

assert module.Document(id="1", fields={}).id == "1"
assert module.UpsertRequest(document=document).document == document
assert module.SearchRequest(query="query").query == "query"
assert module.IndexDefinition(name="products", fields=[]).name == "products"
"#;
    let check = Command::new("python3")
        .args([
            "-c",
            python,
            generated_models.to_str().expect("models path is UTF-8"),
            repository_root.to_str().expect("repository path is UTF-8"),
        ])
        .output()
        .expect("run generated Python regression check");
    let _ = fs::remove_dir_all(&output_root);
    assert!(
        check.status.success(),
        "fresh generated Python check failed:\n{}\n{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
}
