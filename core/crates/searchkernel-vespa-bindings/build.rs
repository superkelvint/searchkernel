mod consumer_bundle;
use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
#[cfg(feature = "native")]
use std::process::{Command, Stdio};

fn support_root(manifest_dir: &Path) -> PathBuf {
    let source_root = manifest_dir.join("../../..");
    if source_root.join("tools/resolve-native-sdk.sh").is_file() {
        return source_root;
    }
    let root = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo must provide OUT_DIR"))
        .join("consumer-support");
    for (relative, contents) in consumer_bundle::FILES {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create consumer support directory");
        }
        fs::write(&path, contents).expect("materialize packaged consumer support file");
    }
    root
}

fn main() {
    if env::var_os("CARGO_FEATURE_NATIVE").is_none() {
        // The workspace's no-default-features lane still builds this member,
        // but must not require the optional Vespa SDK just to compile the
        // checked-in ABI declarations.
        return;
    }

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let support_root = support_root(&manifest_dir);
    #[cfg(feature = "native")]
    if manifest_dir.join("runtime.json").is_file() && env::var_os("SK_NATIVE_SDK").is_none() {
        configure_packaged_consumer_runtime(&manifest_dir, &support_root).unwrap_or_else(|error| {
            panic!("Search Kernel packaged native runtime provisioning failed: {error}")
        });
        return;
    }
    env::set_var("SEARCHKERNEL_PACKAGED_SUPPORT_ROOT", &support_root);
    #[cfg(feature = "native")]
    let target = env::var("TARGET").expect("Cargo must provide TARGET to native build scripts");
    #[cfg(feature = "native")]
    let host = env::var("HOST").expect("Cargo must provide HOST to native build scripts");
    #[cfg(feature = "native")]
    let explicit_sdk = env::var_os("SK_NATIVE_SDK").map(PathBuf::from);
    #[cfg(feature = "native")]
    let hardening_required = env::var_os("CARGO_FEATURE_HARDENING").is_some();
    #[cfg(feature = "native")]
    let mut explicit_sdk_path = None;
    #[cfg(feature = "native")]
    let (sdk, base_sdk) = if let Some(configured) = explicit_sdk {
        let configured = prepare_native_sdk(&manifest_dir, configured, &target);
        explicit_sdk_path = Some(configured.clone());
        if hardening_required {
            let adapter = build_local_adapter(&manifest_dir, &configured, &target, true)
                .unwrap_or_else(|error| {
                    panic!("build hardening Search Kernel Vespa adapter: {error}")
                });
            (adapter, Some(configured))
        } else {
            (configured, None)
        }
    } else {
        // Consumer builds own native provisioning: `ensure` acquires and validates
        // the pinned release artifact into the machine cache when it is absent,
        // stale, or corrupt. SK_NATIVE_SDK remains a maintainer override only.
        let base = resolve_shared_native_sdk(&manifest_dir, &target, "ensure").unwrap_or_else(|error| {
            panic!(
                "Search Kernel native runtime is unavailable or incompatible for target {target}: {error}\nThe supported consumer path provisions the pinned runtime automatically; do not set SK_NATIVE_SDK. Check network access and platform support, then retry."
            )
        });
        let base = fs::canonicalize(&base).unwrap_or_else(|error| {
            panic!(
                "Search Kernel native runtime cache is unreadable after automatic provisioning at '{}': {error}\nRemove no files manually; retry so the resolver can repair the cache.",
                base.display()
            )
        });
        verify_shared_cache_object(&manifest_dir, &base).unwrap_or_else(|error| {
            panic!(
                "Search Kernel native runtime failed compatibility validation after automatic provisioning: {error}\nThe cache is repaired automatically on the next provisioning attempt."
            )
        });
        let adapter = build_local_adapter(&manifest_dir, &base, &target, hardening_required)
            .unwrap_or_else(|error| panic!("build Search Kernel Vespa adapter: {error}"));
        (adapter, Some(base))
    };
    #[cfg(not(feature = "native"))]
    let sdk = fs::canonicalize(default_native_sdk(&manifest_dir))
        .unwrap_or_else(|error| panic!("canonicalize native SDK: {error}"));
    let source_header = support_root.join("core/native/vespa/searchkernel_vespa.h");
    let source_header = fs::canonicalize(&source_header).unwrap_or_else(|error| {
        panic!(
            "canonicalize source native ABI header '{}': {error}",
            source_header.display()
        )
    });
    let using_packaged_sdk = env::var_os("SK_NATIVE_ABI_HEADER").is_none();
    let header = env::var_os("SK_NATIVE_ABI_HEADER")
        .map(PathBuf::from)
        .unwrap_or_else(|| sdk.join("include/searchkernel_vespa.h"));
    if using_packaged_sdk {
        let source_bytes = fs::read(&source_header).unwrap_or_else(|error| {
            panic!(
                "read source native ABI header '{}': {error}",
                source_header.display()
            )
        });
        let sdk_bytes = fs::read(&header).unwrap_or_else(|error| {
            panic!(
                "read packaged native ABI header '{}': {error}",
                header.display()
            )
        });
        if source_bytes != sdk_bytes {
            panic!(
                "packaged native ABI header '{}' differs from source header '{}'; rebuild the SDK from the source header",
                header.display(),
                source_header.display()
            );
        }
    }

    let rust_abi =
        fs::read_to_string(manifest_dir.join("src/abi.rs")).expect("read Rust native ABI version");
    let expected_abi = rust_abi
        .lines()
        .find_map(|line| line.strip_prefix("pub const EXPECTED_ABI: u32 = "))
        .and_then(|value| value.strip_suffix(';'))
        .and_then(|value| value.parse::<u32>().ok())
        .expect("Rust ABI source must define EXPECTED_ABI");
    let header_text = fs::read_to_string(&header).expect("read native ABI header");
    let abi_version = header_text
        .lines()
        .find_map(|line| line.strip_prefix("#define SK_NATIVE_ABI_VERSION "))
        .and_then(|value| value.strip_suffix('U'))
        .and_then(|value| value.parse::<u32>().ok())
        .expect("native ABI header must define SK_NATIVE_ABI_VERSION");
    if abi_version != expected_abi {
        panic!(
            "native ABI header version {abi_version} does not match Rust crate ABI version {expected_abi}"
        );
    }

    #[cfg(feature = "native")]
    {
        if let Err(error) = probe_native_sdk(&sdk, base_sdk.as_deref(), &target, &host) {
            if base_sdk.is_some() {
                panic!(
                    "Search Kernel Vespa adapter '{}' failed its link probe against runtime base:\n{error}",
                    sdk.display()
                );
            }
            println!("cargo:warning=searchkernel: explicit native Vespa SDK link probe failed");
            rebuild_native_sdk(&manifest_dir, &sdk).unwrap_or_else(|rebuild_error| {
                panic!(
                    "explicit native Vespa SDK link probe failed and automatic rebuild could not start:\n{error}\n{rebuild_error}"
                )
            });
            verify_native_provenance(&manifest_dir, &sdk, &target).unwrap_or_else(
                |verify_error| {
                    panic!(
                        "explicit native Vespa SDK remained incompatible after one automatic rebuild:\n{error}\n{verify_error}"
                    )
                },
            );
            probe_native_sdk(&sdk, None, &target, &host).unwrap_or_else(|retry_error| {
                panic!(
                    "explicit native Vespa SDK link probe failed after one automatic rebuild:\n{error}\n{retry_error}"
                )
            });
        }
    }

    #[cfg(feature = "native")]
    if hardening_required {
        verify_hardening_sdk(&manifest_dir, &sdk).unwrap_or_else(|error| {
            panic!(
                "Search Kernel hardening adapter '{}' is missing required failpoint support:\n{error}",
                sdk.display()
            )
        });
    }

    let lib_dir = sdk.join("lib");
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    #[cfg(feature = "native")]
    if let Some(base) = base_sdk.as_deref() {
        println!(
            "cargo:rustc-link-search=native={}",
            base.join("lib").display()
        );
        println!(
            "cargo:rustc-link-arg=-Wl,-rpath,{}",
            base.join("lib").display()
        );
        println!(
            "cargo:rustc-link-arg=-Wl,-rpath-link,{}",
            base.join("lib").display()
        );
    }
    println!("cargo:rustc-link-lib=dylib=searchkernel_vespa");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
    println!("cargo:rerun-if-env-changed=SK_NATIVE_SDK");
    println!("cargo:rerun-if-env-changed=SK_NATIVE_ABI_HEADER");
    println!("cargo:rerun-if-changed={}", source_header.display());
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("../../native/vespa/abi.cc").display()
    );
    for source in [
        "hardening.cc",
        "engine.cc",
        "tensor.cc",
        "errors.cc",
        "query.cc",
        "grouping.cc",
        "search.cc",
        "document.cc",
        "update.cc",
    ] {
        println!(
            "cargo:rerun-if-changed={}",
            manifest_dir
                .join("../../native/vespa")
                .join(source)
                .display()
        );
    }
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir
            .join("../../native/vespa/searchkernel_vespa.map")
            .display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir
            .join("../../native/vespa/searchkernel_vespa_test.map")
            .display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir
            .join("../../native/vespa/searchkernel_vespa_test.h")
            .display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir
            .join("../../../tools/check-native-sdk-hardening.sh")
            .display()
    );
    println!("cargo:rerun-if-env-changed=SK_NATIVE_ENABLE_FAILPOINTS");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_HARDENING");
    println!("cargo:rerun-if-env-changed=SK_NATIVE_EXTRA_CXX_FLAGS");
    println!("cargo:rerun-if-env-changed=SK_NATIVE_EXTRA_LINK_FLAGS");
    println!("cargo:rerun-if-env-changed=SK_NATIVE_SANITIZER");
    println!("cargo:rerun-if-env-changed=SEARCHKERNEL_BUILD_ENV");
    println!("cargo:rerun-if-env-changed=SK_NATIVE_TARGET");
    for name in [
        "PATH",
        "CC",
        "CXX",
        "AR",
        "RANLIB",
        "LD",
        "CFLAGS",
        "CXXFLAGS",
        "CPPFLAGS",
        "LDFLAGS",
        "CMAKE",
        "CMAKE_GENERATOR",
        "CMAKE_PREFIX_PATH",
        "CXXSTDLIB",
        "CRATE_CC_NO_DEFAULTS",
        "RUSTC",
        "VESPA_ROOT",
        "VESPA_DEPS",
        "VESPA_DEPS_LIB_DIR",
        "VESPA_DEPS_INCLUDE_DIR",
        "VESPA_EXTRA_LIB_DIR",
        "VESPA_AWS_LIB_DIR",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    #[cfg(feature = "native")]
    for base in [
        "CC", "CXX", "AR", "RANLIB", "CFLAGS", "CXXFLAGS", "CPPFLAGS", "LDFLAGS",
    ] {
        println!("cargo:rerun-if-env-changed={base}_{}", target);
        println!(
            "cargo:rerun-if-env-changed={base}_{}",
            target.replace('-', "_")
        );
        println!("cargo:rerun-if-env-changed=HOST_{base}");
    }
    println!("cargo:rerun-if-changed=src/abi.rs");
    println!("cargo:rerun-if-changed=src/bindings.rs");
    #[cfg(feature = "native")]
    if let Some(base) = base_sdk.as_deref() {
        let artifact_marker = base.join(".searchkernel-native-base-artifact.json");
        if artifact_marker.exists() {
            // The shared base path is keyed by Vespa checkout, while published
            // generations may advance at that same path. Watching the installed
            // generation receipt makes Cargo rebuild only this local adapter
            // after './dev native-sdk ensure' refreshes the base.
            println!("cargo:rerun-if-changed={}", artifact_marker.display());
        }
    }
    #[cfg(feature = "native")]
    if let Some(external_sdk) = explicit_sdk_path.as_deref() {
        // Explicitly managed full SDKs remain external inputs even when the
        // hardening feature builds a private adapter variant in OUT_DIR.
        println!(
            "cargo:rerun-if-changed={}",
            external_sdk.join("include/searchkernel_vespa.h").display()
        );
        println!(
            "cargo:rerun-if-changed={}",
            external_sdk
                .join("native-build-fingerprint-v2.txt")
                .display()
        );
        println!(
            "cargo:rerun-if-changed={}",
            external_sdk.join("manifest.json").display()
        );
        println!(
            "cargo:rerun-if-changed={}",
            external_sdk.join("lib/libsearchkernel_vespa.so").display()
        );
    }

    #[cfg(feature = "native-contract-tests")]
    compile_contract_tests(&manifest_dir, &sdk);
}

#[cfg(not(feature = "native"))]
fn default_native_sdk(manifest_dir: &Path) -> PathBuf {
    // OUT_DIR is the one Cargo-provided path that reflects an explicit
    // CARGO_TARGET_DIR override. Deriving from it keeps custom target roots
    // isolated without requiring developers to export a second variable.
    if let Some(out_dir) = env::var_os("OUT_DIR") {
        let out_dir = PathBuf::from(out_dir);
        if let Some(target_root) = out_dir.ancestors().nth(4) {
            return target_root.join("native-sdk/linux-x86_64");
        }
    }
    manifest_dir.join("../../native/sdk/linux-x86_64")
}

#[cfg(feature = "native")]
fn prepare_native_sdk(manifest_dir: &Path, configured: PathBuf, target: &str) -> PathBuf {
    let root = env::var_os("SEARCHKERNEL_PACKAGED_SUPPORT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("../../.."));
    let configured = if configured.is_absolute() {
        configured
    } else {
        root.join(configured)
    };
    let mut last_error = String::new();
    for attempt in 0..=1 {
        if let Ok(sdk) = fs::canonicalize(&configured) {
            match verify_native_provenance(manifest_dir, &sdk, target) {
                Ok(()) => return sdk,
                Err(error) => last_error = error,
            }
        } else {
            last_error = format!("native SDK '{}' does not exist", configured.display());
        }
        if attempt == 0 {
            println!(
                "cargo:warning=searchkernel: explicitly managed native SDK is missing or stale; rebuilding it once"
            );
            rebuild_native_sdk(manifest_dir, &configured).unwrap_or_else(|error| {
                panic!(
                    "explicit native Vespa SDK is missing or stale and automatic rebuild failed:\n{last_error}\n{error}"
                )
            });
        }
    }
    panic!("explicit native Vespa SDK is incompatible after one automatic rebuild:\n{last_error}")
}

#[cfg(feature = "native")]
fn resolve_shared_native_sdk(
    manifest_dir: &Path,
    target: &str,
    mode: &str,
) -> Result<PathBuf, String> {
    let root = env::var_os("SEARCHKERNEL_PACKAGED_SUPPORT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("../../.."));
    let output = Command::new("bash")
        .arg(root.join("tools/resolve-native-sdk.sh"))
        .arg(mode)
        .env("SK_NATIVE_TARGET", target)
        .output()
        .map_err(|error| format!("spawn shared native SDK resolver: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "shared native SDK resolver exited with {}\nstdout={}\nstderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let path = stdout
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| "shared native SDK resolver returned no path".to_owned())?;
    Ok(PathBuf::from(path))
}

#[cfg(feature = "native")]
fn configure_packaged_consumer_runtime(
    manifest_dir: &Path,
    support_root: &Path,
) -> Result<(), String> {
    if env::var_os("CARGO_FEATURE_HARDENING").is_some() {
        return Err("hardening is a maintainer-only native build mode and is unavailable from the consumer runtime".to_owned());
    }
    let resolver = support_root.join("tools/consumer_runtime.py");
    let descriptor = manifest_dir.join("runtime.json");
    let output = Command::new("python3")
        .arg(&resolver)
        .arg("ensure")
        .arg(&descriptor)
        .output()
        .map_err(|error| {
            format!(
                "start consumer-runtime resolver '{}': {error}",
                resolver.display()
            )
        })?;
    if !output.status.success() {
        return Err(format!(
            "consumer-runtime resolver exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let runtime = PathBuf::from(
        String::from_utf8(output.stdout)
            .map_err(|error| format!("resolver returned non-UTF-8 path: {error}"))?
            .trim(),
    );
    let lib_dir = runtime.join("lib");
    let adapter = lib_dir.join("libsearchkernel_vespa.so.2");
    if !adapter.is_file() {
        return Err(format!(
            "validated consumer runtime is missing '{}'",
            adapter.display()
        ));
    }
    let out_dir = PathBuf::from(
        env::var_os("OUT_DIR").ok_or_else(|| "Cargo did not provide OUT_DIR".to_owned())?,
    );
    stage_consumer_runtime_for_cargo_outputs(&out_dir, &lib_dir)?;
    // The immutable runtime intentionally carries the ABI library by SONAME only
    // (`.so.2`). Give the linker its conventional unversioned filename without
    // mutating the validated runtime object.
    let link_dir = out_dir.join("consumer-runtime-link");
    fs::create_dir_all(&link_dir)
        .map_err(|error| format!("create consumer runtime link directory: {error}"))?;
    fs::copy(&adapter, link_dir.join("libsearchkernel_vespa.so"))
        .map_err(|error| format!("stage consumer runtime linker input: {error}"))?;
    println!("cargo:rustc-link-search=native={}", link_dir.display());
    println!("cargo:rustc-link-lib=dylib=searchkernel_vespa");
    println!("cargo:rerun-if-changed={}", descriptor.display());
    println!("cargo:rerun-if-env-changed=SEARCHKERNEL_RUNTIME_DIR");
    println!("cargo:rerun-if-env-changed=SEARCHKERNEL_RUNTIME_CACHE");
    println!("cargo:rerun-if-env-changed=SEARCHKERNEL_RUNTIME_ORIGIN");
    Ok(())
}

#[cfg(feature = "native")]
fn stage_consumer_runtime_for_cargo_outputs(out_dir: &Path, lib_dir: &Path) -> Result<(), String> {
    let profile_dir = out_dir
        .ancestors()
        .nth(3)
        .ok_or_else(|| format!("unexpected Cargo OUT_DIR layout: {}", out_dir.display()))?;
    for executable_dir in [
        profile_dir.to_path_buf(),
        profile_dir.join("deps"),
        profile_dir.join("examples"),
    ] {
        let runtime_dir = executable_dir.join(".searchkernel-native");
        fs::create_dir_all(&runtime_dir).map_err(|error| {
            format!(
                "create packaged native runtime directory '{}': {error}",
                runtime_dir.display()
            )
        })?;
        for entry in fs::read_dir(lib_dir)
            .map_err(|error| format!("read consumer runtime '{}': {error}", lib_dir.display()))?
        {
            let entry = entry.map_err(|error| format!("read consumer runtime entry: {error}"))?;
            let ty = entry
                .file_type()
                .map_err(|error| format!("inspect consumer runtime entry: {error}"))?;
            if !ty.is_file() && !ty.is_symlink() {
                continue;
            }
            let destination = runtime_dir.join(entry.file_name());
            fs::copy(entry.path(), &destination).map_err(|error| {
                format!(
                    "stage consumer runtime '{}' -> '{}': {error}",
                    entry.path().display(),
                    destination.display()
                )
            })?;
        }
    }
    Ok(())
}

#[cfg(feature = "native")]
fn build_local_adapter(
    manifest_dir: &Path,
    base_sdk: &Path,
    target: &str,
    hardening_required: bool,
) -> Result<PathBuf, String> {
    let root = env::var_os("SEARCHKERNEL_PACKAGED_SUPPORT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("../../.."));
    let out_dir = PathBuf::from(
        env::var_os("OUT_DIR").ok_or_else(|| "Cargo did not provide OUT_DIR".to_owned())?,
    );
    let adapter = out_dir.join("native-adapter");
    let build = out_dir.join("native-adapter-build");

    let vespa_root = resolve_shared_native_sdk(manifest_dir, target, "vespa-root")?;
    let output = Command::new("bash")
        .arg(root.join("core/native/vespa/build-sdk.sh"))
        .env("VESPA_ROOT", vespa_root)
        .env("SK_NATIVE_SDK", &adapter)
        .env("SK_NATIVE_BUILD_DIR", &build)
        .env("SK_NATIVE_TARGET", target)
        .env("SK_NATIVE_ADAPTER_ONLY", "1")
        // Cargo does not propagate dependency build-script rustc-link-arg values to
        // the consuming application final link. Encode an $ORIGIN-qualified SONAME
        // instead, then stage the adapter beside each normal Cargo executable dir.
        // The resulting DT_NEEDED entry is self-resolving for downstream consumers.
        .env(
            "SK_NATIVE_ADAPTER_SONAME",
            "$ORIGIN/.searchkernel-native/libsearchkernel_vespa.so.2",
        )
        .env("SK_NATIVE_BASE_SDK", base_sdk)
        .env(
            "SK_NATIVE_ENABLE_FAILPOINTS",
            if hardening_required { "1" } else { "0" },
        )
        .env("SK_NATIVE_REGENERATE_BINDINGS", "0")
        .env("SK_NATIVE_SKIP_BINDINGS_CHECK", "1")
        .env(
            "SK_NATIVE_SKIP_ABI_AUTHORITIES",
            if root.join(".git").exists() { "0" } else { "1" },
        )
        .output()
        .map_err(|error| format!("spawn adapter-only native build: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "adapter-only native build exited with {}\nstdout={}\nstderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    stage_adapter_for_cargo_outputs(&out_dir, &adapter)?;
    fs::canonicalize(&adapter)
        .map_err(|error| format!("canonicalize adapter '{}': {error}", adapter.display()))
}

#[cfg(feature = "native")]
fn stage_adapter_for_cargo_outputs(out_dir: &Path, adapter: &Path) -> Result<(), String> {
    let profile_dir = out_dir
        .ancestors()
        .nth(3)
        .ok_or_else(|| format!("unexpected Cargo OUT_DIR layout: {}", out_dir.display()))?;
    let source = adapter.join("lib/libsearchkernel_vespa.so.2");
    if !source.is_file() {
        return Err(format!(
            "native adapter build did not produce '{}'",
            source.display()
        ));
    }

    // Cargo places ordinary binaries at <profile>/ and test/bench binaries in
    // <profile>/deps. Examples have their own executable directory. The SONAME
    // uses $ORIGIN, so stage the same adapter at each possible executable origin.
    for executable_dir in [
        profile_dir.to_path_buf(),
        profile_dir.join("deps"),
        profile_dir.join("examples"),
    ] {
        let runtime_dir = executable_dir.join(".searchkernel-native");
        fs::create_dir_all(&runtime_dir).map_err(|error| {
            format!(
                "create native runtime directory '{}': {error}",
                runtime_dir.display()
            )
        })?;
        let destination = runtime_dir.join("libsearchkernel_vespa.so.2");
        fs::copy(&source, &destination).map_err(|error| {
            format!(
                "stage native adapter '{}' -> '{}': {error}",
                source.display(),
                destination.display()
            )
        })?;
    }
    Ok(())
}

#[cfg(feature = "native")]
fn verify_shared_cache_object(manifest_dir: &Path, sdk: &Path) -> Result<(), String> {
    if sdk.join("lib/libsearchkernel_vespa.so").exists()
        || sdk.join("include/searchkernel_vespa.h").exists()
    {
        return Err(format!(
            "shared Vespa base '{}' contains Search Kernel adapter artifacts",
            sdk.display()
        ));
    }

    let root = env::var_os("SEARCHKERNEL_PACKAGED_SUPPORT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("../../.."));
    let source_manifest_path = root.join("core/native/vespa/manifest.json");
    let source_manifest = fs::read_to_string(&source_manifest_path).map_err(|error| {
        format!(
            "read pinned Vespa manifest '{}': {error}",
            source_manifest_path.display()
        )
    })?;
    let pinned = manifest_string_field(&source_manifest, "vespa_commit").ok_or_else(|| {
        format!(
            "pinned Vespa manifest '{}' has no vespa_commit",
            source_manifest_path.display()
        )
    })?;

    let manifest_path = sdk.join("manifest.json");
    let manifest = fs::read_to_string(&manifest_path).map_err(|error| {
        format!(
            "read shared Vespa base manifest '{}': {error}",
            manifest_path.display()
        )
    })?;
    if !manifest.contains(r#""kind": "vespa-base""#) && !manifest.contains(r#""kind":"vespa-base""#)
    {
        return Err(format!(
            "shared Vespa base manifest '{}' is not kind=vespa-base",
            manifest_path.display()
        ));
    }
    let recorded_checkout = manifest_string_field(&manifest, "vespa_commit").ok_or_else(|| {
        format!(
            "shared Vespa base manifest '{}' has no vespa_commit",
            manifest_path.display()
        )
    })?;
    if recorded_checkout != pinned {
        return Err(format!(
            "shared Vespa base '{}' is for checkout {recorded_checkout}, expected {pinned}",
            sdk.display()
        ));
    }

    let object_name = sdk
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            format!(
                "shared Vespa base path '{}' has no checkout key",
                sdk.display()
            )
        })?;
    if object_name != pinned {
        return Err(format!(
            "shared Vespa base object '{}' does not match pinned checkout {pinned}",
            sdk.display()
        ));
    }
    Ok(())
}

#[cfg(feature = "native")]
fn manifest_string_field<'a>(manifest: &'a str, field: &str) -> Option<&'a str> {
    let key = format!("\"{field}\"");
    let (_, after_key) = manifest.split_once(&key)?;
    let (_, after_colon) = after_key.split_once(':')?;
    let value = after_colon.trim_start().strip_prefix('"')?;
    value.split_once('"').map(|(value, _)| value)
}

#[cfg(feature = "native")]
fn rebuild_native_sdk(manifest_dir: &Path, sdk: &Path) -> Result<(), String> {
    let root = env::var_os("SEARCHKERNEL_PACKAGED_SUPPORT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("../../.."));
    let output = Command::new("bash")
        .arg(root.join("core/native/vespa/build-sdk.sh"))
        .env("SK_NATIVE_SDK", sdk)
        .env("SK_NATIVE_TARGET", env::var("TARGET").unwrap_or_default())
        .env("SK_NATIVE_ENABLE_FAILPOINTS", "0")
        .env("SK_NATIVE_REGENERATE_BINDINGS", "0")
        .env("SK_NATIVE_SKIP_BINDINGS_CHECK", "1")
        .output()
        .map_err(|error| format!("spawn native SDK rebuild: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "native SDK rebuild exited with {}\nstdout={}\nstderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

#[cfg(feature = "native")]
fn verify_hardening_sdk(manifest_dir: &Path, sdk: &Path) -> Result<(), String> {
    let root = env::var_os("SEARCHKERNEL_PACKAGED_SUPPORT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("../../.."));
    let output = Command::new("bash")
        .arg(root.join("tools/check-native-sdk-hardening.sh"))
        .arg(sdk)
        .output()
        .map_err(|error| format!("run native hardening SDK verifier: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "native hardening SDK verifier exited with {}\nstdout={}\nstderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

#[cfg(feature = "native")]
fn verify_native_provenance(manifest_dir: &Path, sdk: &Path, target: &str) -> Result<(), String> {
    let fingerprint_path = sdk.join("native-build-fingerprint-v2.txt");
    let recorded = fs::read_to_string(&fingerprint_path).map_err(|error| {
        format!(
            "read native SDK provenance '{}': {error}",
            fingerprint_path.display()
        )
    })?;
    let recorded_digest = fingerprint_digest(&recorded).ok_or_else(|| {
        format!(
            "native SDK provenance '{}' is malformed",
            fingerprint_path.display()
        )
    })?;
    let root = env::var_os("SEARCHKERNEL_PACKAGED_SUPPORT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("../../.."));
    let vespa_root = env::var_os("VESPA_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("upstream/vespa"));
    let output = Command::new("python3")
        .arg(root.join("tools/native-build-fingerprint.py"))
        .arg("--root")
        .arg(&root)
        .arg("--vespa-root")
        .arg(vespa_root)
        .arg("--target")
        .arg(target)
        .output()
        .map_err(|error| format!("run native build fingerprint verifier: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "native build fingerprint verifier failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let current = String::from_utf8_lossy(&output.stdout);
    let current_digest = fingerprint_digest(&current)
        .ok_or_else(|| "native build fingerprint verifier returned malformed output".to_owned())?;
    if current_digest != recorded_digest {
        return Err(format!(
            "native SDK '{}' was built for a different native environment\nrecorded fingerprint: {recorded_digest}\ncurrent fingerprint: {current_digest}",
            sdk.display()
        ));
    }
    Ok(())
}

#[cfg(feature = "native")]
fn fingerprint_digest(text: &str) -> Option<&str> {
    text.lines()
        .find_map(|line| line.strip_prefix("sha256="))
        .filter(|digest| digest.len() == 64)
}

#[cfg(feature = "native")]
fn probe_native_sdk(
    sdk: &Path,
    base_sdk: Option<&Path>,
    target: &str,
    host: &str,
) -> Result<(), String> {
    let probe_dir =
        PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set")).join("sdk-probe");
    fs::create_dir_all(&probe_dir)
        .map_err(|error| format!("create native SDK compatibility probe directory: {error}"))?;
    let source = probe_dir.join("native_sdk_link_probe.cc");
    let object = probe_dir.join("native_sdk_link_probe.o");
    let executable = probe_dir.join("native_sdk_link_probe");
    fs::write(
        &source,
        "#include <searchkernel_vespa.h>\nint main() { return static_cast<int>(sk_native_abi_version()); }\n",
    )
    .map_err(|error| format!("write native SDK compatibility probe: {error}"))?;

    let mut config = cc::Build::new();
    config
        .cpp(true)
        .cargo_metadata(false)
        .target(target)
        .host(host);
    let compiler = config.get_compiler();
    let mut compile = compiler.to_command();
    compile
        .arg("-std=c++17")
        .arg("-I")
        .arg(sdk.join("include"))
        .args(split_flags("CPPFLAGS"))
        .args(split_flags("CXXFLAGS"))
        .args(split_flags("SK_NATIVE_EXTRA_CXX_FLAGS"))
        .arg("-c")
        .arg(&source)
        .arg("-o")
        .arg(&object);
    run_probe_command(&mut compile, "compile native SDK compatibility probe")?;

    let mut link = compiler.to_command();
    link.arg(&object)
        .arg("-L")
        .arg(sdk.join("lib"))
        .arg(format!("-Wl,-rpath,{}", sdk.join("lib").display()))
        .arg(format!("-Wl,-rpath-link,{}", sdk.join("lib").display()));
    if let Some(base) = base_sdk {
        link.arg("-L")
            .arg(base.join("lib"))
            .arg(format!("-Wl,-rpath,{}", base.join("lib").display()))
            .arg(format!("-Wl,-rpath-link,{}", base.join("lib").display()));
    }
    link.arg("-Wl,--no-as-needed")
        .args(split_flags("LDFLAGS"))
        .args(split_flags("SK_NATIVE_EXTRA_LINK_FLAGS"))
        .arg("-lsearchkernel_vespa");
    let standard_library = env::var("CXXSTDLIB").unwrap_or_else(|_| {
        if target.contains("linux") {
            "stdc++".to_owned()
        } else if target.contains("apple") || target.contains("darwin") {
            "c++".to_owned()
        } else {
            String::new()
        }
    });
    if !standard_library.is_empty() {
        link.arg(format!("-l{standard_library}"));
    }
    link.arg("-o").arg(&executable);
    run_probe_command(&mut link, "link native SDK compatibility probe")
}

#[cfg(feature = "native")]
fn split_flags(variable: &str) -> Vec<String> {
    env::var(variable)
        .unwrap_or_default()
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

#[cfg(feature = "native")]
fn run_probe_command(command: &mut Command, description: &str) -> Result<(), String> {
    let output = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("{description}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "{description} failed with {}\nstdout={}\nstderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

#[cfg(feature = "native-contract-tests")]
fn compile_contract_tests(manifest_dir: &Path, sdk: &Path) {
    let verifier_dir = manifest_dir.join("../../../tools/native-contract-tests");
    println!(
        "cargo:rerun-if-changed={}",
        verifier_dir.join("schema_identity_fd_test.cc").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        verifier_dir.join("transport_lifecycle_test.cc").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        verifier_dir.join("query_lifetime_test.cc").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        verifier_dir.join("abi_noexcept_check.cc").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        verifier_dir
            .join("native_header_namespace_check.cc")
            .display()
    );
    println!("cargo:rerun-if-changed=../../native/vespa/file_descriptor.h");
    println!("cargo:rerun-if-changed=../../native/vespa/transport_owner.h");
    cc::Build::new()
        .cpp(true)
        .file(verifier_dir.join("schema_identity_fd_test.cc"))
        .include(manifest_dir.join("../../native/vespa"))
        .warnings(true)
        .compile("searchkernel_schema_identity_fd_test");
    cc::Build::new()
        .cpp(true)
        .file(verifier_dir.join("transport_lifecycle_test.cc"))
        .include(manifest_dir.join("../../native/vespa"))
        .warnings(true)
        .compile("searchkernel_transport_lifecycle_test");
    cc::Build::new()
        .cpp(true)
        .file(verifier_dir.join("query_lifetime_test.cc"))
        .include(sdk.join("include"))
        .warnings(true)
        .compile("searchkernel_query_lifetime_test");
    cc::Build::new()
        .cpp(true)
        .file(verifier_dir.join("abi_noexcept_check.cc"))
        .include(sdk.join("include"))
        .warnings(true)
        .compile("searchkernel_vespa_abi_noexcept");
    cc::Build::new()
        .cpp(true)
        .file(verifier_dir.join("native_header_namespace_check.cc"))
        .include(sdk.join("include"))
        .warnings(true)
        .compile("searchkernel_vespa_header_namespace");
}
