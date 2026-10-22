#[cfg(feature = "json")]
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

#[cfg(feature = "json")]
const FLATBUFFERS_VERSION: &str = "25.12.19";

#[cfg(feature = "json")]
fn print_native_inputs() {
    println!("cargo:rerun-if-changed=native");
    println!("cargo:rerun-if-changed=bfbs/searchkernel_request.bfbs");
    println!("cargo:rerun-if-changed=bfbs/searchkernel_response.bfbs");
    println!("cargo:rerun-if-env-changed=FLATBUFFERS_CPP_ROOT");
    println!("cargo:rerun-if-env-changed=FLATC_PATH");
    println!("cargo:rerun-if-env-changed=SEARCHKERNEL_BUILD_ENV");

    for name in [
        "PATH",
        "CC",
        "CXX",
        "AR",
        "RANLIB",
        "CFLAGS",
        "CXXFLAGS",
        "CPPFLAGS",
        "LDFLAGS",
        "CXXSTDLIB",
        "CRATE_CC_NO_DEFAULTS",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }

    let target = env::var("TARGET").unwrap_or_default();
    let target_underscore = target.replace('-', "_");
    for base in [
        "CC", "CXX", "AR", "RANLIB", "CFLAGS", "CXXFLAGS", "CPPFLAGS", "LDFLAGS",
    ] {
        for name in [
            format!("{base}_{target}"),
            format!("{base}_{target_underscore}"),
            format!("HOST_{base}"),
        ] {
            println!("cargo:rerun-if-env-changed={name}");
        }
    }
}

#[cfg(not(feature = "json"))]
fn main() {}

#[cfg(feature = "json")]
fn main() {
    print_native_inputs();

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let packaged = manifest_dir.join("schema/searchkernel.fbs");
    let schema = if packaged.is_file() {
        packaged
    } else {
        manifest_dir.join("../../../api/searchkernel.fbs")
    };
    println!("cargo:rerun-if-changed={}", schema.display());
    fs::copy(
        schema,
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("searchkernel.fbs"),
    )
    .expect("stage canonical schema for the standard JSON bridge");

    let root = resolve_flatbuffers_root().unwrap_or_else(|error| {
        panic!(
            "{error}\nInstall the pinned FlatBuffers toolchain once with './dev install flatbuffers'. \
             Cargo does not build FlatBuffers itself."
        )
    });
    let include = root.join("include");
    let library = root.join("lib");
    verify_flatbuffers_version(&include).unwrap_or_else(|error| {
        panic!("FlatBuffers prerequisite at '{}': {error}", root.display())
    });

    let static_library = library.join(if cfg!(target_os = "windows") {
        "flatbuffers.lib"
    } else {
        "libflatbuffers.a"
    });
    if !static_library.is_file() {
        panic!(
            "FlatBuffers {FLATBUFFERS_VERSION} static library is missing at '{}'. \
             Run './dev install flatbuffers' once; Cargo will not rebuild it.",
            static_library.display()
        );
    }

    let mut bridge = cc::Build::new();
    bridge
        .cpp(true)
        .file("native/flatbuffers_json_bridge.cc")
        .include("native")
        .include(&include)
        .flag_if_supported("-std=c++17")
        .warnings(true);
    let compiler = bridge.get_compiler();
    bridge.compile("searchkernel_flatbuffers_json_bridge");

    println!("cargo:rustc-link-search=native={}", library.display());
    println!("cargo:rustc-link-lib=static=flatbuffers");
    if cfg!(target_os = "linux") {
        // Rust's cc linker can otherwise find the host OS libstdc++ (GCC 8)
        // after cc compiled this bridge with the CI toolset (GCC 15).
        // Query the actual C++ compiler so the link uses its matching runtime.
        let output = compiler
            .to_command()
            .arg("-print-file-name=libstdc++.so")
            .output()
            .expect("query the C++ compiler for libstdc++");
        if !output.status.success() {
            panic!("C++ compiler could not locate libstdc++");
        }
        let runtime = String::from_utf8(output.stdout).expect("libstdc++ path is UTF-8");
        let runtime = Path::new(runtime.trim());
        if !runtime.is_absolute() || !runtime.is_file() {
            panic!(
                "C++ compiler did not report an installed libstdc++: {}",
                runtime.display()
            );
        }
        let runtime_dir = runtime.parent().expect("libstdc++ has a parent directory");
        println!("cargo:rustc-link-search=native={}", runtime_dir.display());
        println!("cargo:rustc-link-lib=dylib=stdc++");
    } else if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-lib=dylib=c++");
    }
}

#[cfg(feature = "json")]
fn resolve_flatbuffers_root() -> Result<PathBuf, String> {
    if let Some(explicit) = env::var_os("FLATBUFFERS_CPP_ROOT") {
        return require_root(PathBuf::from(explicit), "FLATBUFFERS_CPP_ROOT");
    }

    if let Some(flatc) = env::var_os("FLATC_PATH") {
        let flatc = PathBuf::from(flatc);
        let prefix = flatc.parent().and_then(Path::parent).ok_or_else(|| {
            format!(
                "FLATC_PATH '{}' has no installation prefix",
                flatc.display()
            )
        })?;
        return require_root(prefix.to_path_buf(), "FLATC_PATH");
    }

    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR")
            .ok_or_else(|| "CARGO_MANIFEST_DIR is not set".to_owned())?,
    );
    let repository_root = manifest_dir.ancestors().nth(3).ok_or_else(|| {
        format!(
            "cannot derive repository root from '{}'",
            manifest_dir.display()
        )
    })?;

    if let Some(common) = git_common_dir(repository_root) {
        let build_env = env::var("SEARCHKERNEL_BUILD_ENV").unwrap_or_else(|_| "host".to_owned());
        let shared = common
            .join("searchkernel/toolchains/flatbuffers")
            .join(FLATBUFFERS_VERSION)
            .join(build_env);
        if root_is_usable(&shared) {
            return Ok(shared);
        }
    }

    for candidate in [PathBuf::from("/usr/local"), PathBuf::from("/usr")] {
        if root_is_usable(&candidate) {
            return Ok(candidate);
        }
    }

    Err(format!(
        "FlatBuffers {FLATBUFFERS_VERSION} C++ prerequisite was not found in the clone-wide toolchain or system prefixes"
    ))
}

#[cfg(feature = "json")]
fn git_common_dir(repository_root: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository_root)
        .args(["rev-parse", "--git-common-dir"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8(output.stdout).ok()?;
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let path = PathBuf::from(raw);
    let path = if path.is_absolute() {
        path
    } else {
        repository_root.join(path)
    };
    fs::canonicalize(path).ok()
}

#[cfg(feature = "json")]
fn require_root(root: PathBuf, source: &str) -> Result<PathBuf, String> {
    if root_is_usable(&root) {
        Ok(root)
    } else {
        Err(format!(
            "{source} points to '{}', but that prefix does not contain FlatBuffers {FLATBUFFERS_VERSION} headers and static library",
            root.display()
        ))
    }
}

#[cfg(feature = "json")]
fn root_is_usable(root: &Path) -> bool {
    let include = root.join("include/flatbuffers/base.h");
    let library = root.join("lib");
    let archive = if cfg!(target_os = "windows") {
        library.join("flatbuffers.lib")
    } else {
        library.join("libflatbuffers.a")
    };
    include.is_file()
        && archive.is_file()
        && verify_flatbuffers_version(&root.join("include")).is_ok()
}

#[cfg(feature = "json")]
fn verify_flatbuffers_version(include: &Path) -> Result<(), String> {
    let header = fs::read_to_string(include.join("flatbuffers/base.h"))
        .map_err(|error| format!("read FlatBuffers version header: {error}"))?;
    let major = header
        .lines()
        .find_map(|line| line.strip_prefix("#define FLATBUFFERS_VERSION_MAJOR "))
        .ok_or_else(|| "FlatBuffers major version is missing".to_owned())?;
    let minor = header
        .lines()
        .find_map(|line| line.strip_prefix("#define FLATBUFFERS_VERSION_MINOR "))
        .ok_or_else(|| "FlatBuffers minor version is missing".to_owned())?;
    let revision = header
        .lines()
        .find_map(|line| line.strip_prefix("#define FLATBUFFERS_VERSION_REVISION "))
        .ok_or_else(|| "FlatBuffers revision is missing".to_owned())?;
    let actual = format!("{major}.{minor}.{revision}");
    if actual == FLATBUFFERS_VERSION {
        Ok(())
    } else {
        Err(format!("expected {FLATBUFFERS_VERSION}, got {actual}"))
    }
}
