use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use ring::digest::{Context, SHA256};

pub const FINGERPRINT_VERSION: &str = "1";
pub const FLATBUFFERS_VERSION: &str = "25.12.19";
pub const FLATBUFFERS_SOURCE_URL: &str =
    "https://github.com/google/flatbuffers/archive/refs/tags/v25.12.19.tar.gz";
pub const FLATBUFFERS_SOURCE_SHA256: &str =
    "f81c3162b1046fe8b84b9a0dbdd383e24fdbcf88583b9cb6028f90d04d90696a";
pub const BUILDER_REVISION: &str = "searchkernel-flatbuffers-build-1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeBuildFingerprint {
    pub fields: BTreeMap<String, String>,
}

impl NativeBuildFingerprint {
    pub fn current() -> Self {
        let target = env::var("TARGET").unwrap_or_else(|_| "unknown-target".into());
        let host = env::var("HOST").unwrap_or_else(|_| "unknown-host".into());
        let c = compiler("cc", &target, &host);
        let cxx = compiler("cxx", &target, &host);
        let cmake = configured_tool("CMAKE", "cmake");
        let mut fields = BTreeMap::new();

        add(&mut fields, "fingerprint.version", FINGERPRINT_VERSION);
        add(&mut fields, "builder.revision", BUILDER_REVISION);
        add(&mut fields, "flatbuffers.version", FLATBUFFERS_VERSION);
        add(
            &mut fields,
            "flatbuffers.source_url",
            FLATBUFFERS_SOURCE_URL,
        );
        add(
            &mut fields,
            "flatbuffers.source_sha256",
            FLATBUFFERS_SOURCE_SHA256,
        );
        add(&mut fields, "cargo.host", &host);
        add(&mut fields, "cargo.target", &target);
        add(&mut fields, "profile", &env_value("PROFILE"));
        add(&mut fields, "opt_level", &env_value("OPT_LEVEL"));
        add(&mut fields, "debug", &env_value("DEBUG"));
        add(
            &mut fields,
            "build_environment",
            &env_value("SEARCHKERNEL_BUILD_ENV"),
        );
        add(
            &mut fields,
            "rustc",
            &command_identity("RUSTC", "rustc", &["-vV"]),
        );
        add(&mut fields, "cmake", &cmake);
        add(&mut fields, "c.compiler", &c.identity);
        add(&mut fields, "c.compiler_args", &c.args);
        add(&mut fields, "cxx.compiler", &cxx.identity);
        add(&mut fields, "cxx.compiler_args", &cxx.args);
        for (name, default) in [("ar", "ar"), ("ranlib", "ranlib"), ("ld", "ld")] {
            add(
                &mut fields,
                name,
                &configured_tool(name.to_ascii_uppercase().as_str(), default),
            );
        }
        add(&mut fields, "libc", &libc_identity());
        add(&mut fields, "cxx_runtime", &cxx_runtime_identity(&cxx.path));

        for name in relevant_environment_names(&target) {
            add(&mut fields, &format!("env.{name}"), &env_value(&name));
        }

        Self { fields }
    }

    pub fn canonical(&self) -> String {
        self.fields
            .iter()
            .map(|(key, value)| format!("{}\t{}\n", escape(key), escape(value)))
            .collect()
    }

    pub fn digest(&self) -> String {
        let mut context = Context::new(&SHA256);
        context.update(self.canonical().as_bytes());
        context
            .finish()
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    pub fn changed_fields(&self, previous: &Self) -> Vec<String> {
        self.fields
            .keys()
            .chain(previous.fields.keys())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .filter(|key| self.fields.get(*key) != previous.fields.get(*key))
            .cloned()
            .collect()
    }

    pub fn read(path: &Path) -> Option<Self> {
        let text = fs::read_to_string(path).ok()?;
        if text.lines().next()? != format!("fingerprint-version={FINGERPRINT_VERSION}") {
            return None;
        }
        let mut fields = BTreeMap::new();
        for line in text.lines().skip(2) {
            let (key, value) = line.split_once('\t')?;
            fields.insert(unescape(key), unescape(value));
        }
        let fingerprint = Self { fields };
        let recorded_digest = text.lines().nth(1)?.strip_prefix("sha256=")?;
        (recorded_digest == fingerprint.digest()).then_some(fingerprint)
    }

    pub fn write(&self, path: &Path) -> std::io::Result<()> {
        let mut text = format!(
            "fingerprint-version={}\nsha256={}\n",
            FINGERPRINT_VERSION,
            self.digest()
        );
        text.push_str(&self.canonical());
        fs::write(path, text)
    }
}

#[derive(Clone, Debug)]
pub struct CompilerIdentity {
    pub path: PathBuf,
    pub identity: String,
    pub args: String,
}

pub fn compiler(kind: &str, target: &str, host: &str) -> CompilerIdentity {
    let mut build = cc::Build::new();
    build
        .cpp(kind == "cxx")
        .cargo_metadata(false)
        .target(target)
        .host(host);
    let tool = build.get_compiler();
    let path = resolve_program(tool.path());
    let identity = format!(
        "path={}\nversion={}\ndumpmachine={}",
        path.display(),
        command_output(&path, &["--version"]),
        command_output(&path, &["-dumpmachine"])
    );
    let args = tool
        .args()
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ");
    CompilerIdentity {
        path,
        identity,
        args,
    }
}

pub fn invalidate_native_output(out_dir: &Path) -> std::io::Result<()> {
    for name in ["build", "bin", "include", "lib", "probe"] {
        let path = out_dir.join(name);
        if path.exists() {
            fs::remove_dir_all(path)?;
        }
    }
    let fingerprint = out_dir.join("native-build-fingerprint-v1.txt");
    if fingerprint.exists() {
        fs::remove_file(fingerprint)?;
    }
    Ok(())
}

pub fn native_output_exists(out_dir: &Path) -> bool {
    out_dir.join("bin/flatc").is_file()
        && out_dir.join("include/flatbuffers/idl.h").is_file()
        && [
            "lib/libflatbuffers.a",
            "lib/flatbuffers.lib",
            "lib/libflatbuffers.lib",
        ]
        .iter()
        .map(|name| out_dir.join(name))
        .any(|path| path.is_file())
}

pub fn fingerprint_path(out_dir: &Path) -> PathBuf {
    out_dir.join("native-build-fingerprint-v1.txt")
}

pub fn configured_tool(variable: &str, default: &str) -> String {
    let requested = env::var(variable).unwrap_or_else(|_| default.into());
    let path = resolve_program(Path::new(&requested));
    format!(
        "path={}\nversion={}",
        path.display(),
        command_output(&path, &["--version"])
    )
}

pub fn resolve_program(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    }
    env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| env::split_paths(&paths).collect::<Vec<_>>())
        .map(|directory| directory.join(path))
        .find(|candidate| candidate.is_file())
        .map(|candidate| fs::canonicalize(&candidate).unwrap_or(candidate))
        .unwrap_or_else(|| path.to_path_buf())
}

pub fn relevant_environment_names(target: &str) -> Vec<String> {
    let target_underscore = target.replace('-', "_");
    let mut names = vec![
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
        "CMAKE_ARGS",
        "CMAKE_GENERATOR",
        "CMAKE_PREFIX_PATH",
        "CMAKE_TOOLCHAIN_FILE",
        "CMAKE_BUILD_TYPE",
        "CMAKE_C_FLAGS",
        "CMAKE_CXX_FLAGS",
        "CMAKE_EXE_LINKER_FLAGS",
        "CMAKE_SHARED_LINKER_FLAGS",
        "CMAKE_MODULE_LINKER_FLAGS",
        "CMAKE_MAKE_PROGRAM",
        "CMAKE_INSTALL_PREFIX",
        "CMAKE_BUILD_PARALLEL_LEVEL",
        "CMAKE_SYSTEM_NAME",
        "CMAKE_SYSTEM_PROCESSOR",
        "BUILD_SHARED_LIBS",
        "FLATBUFFERS_BUILD_BENCHMARKS",
        "FLATBUFFERS_BUILD_CPP17",
        "FLATBUFFERS_BUILD_FLATC",
        "FLATBUFFERS_BUILD_FLATHASH",
        "FLATBUFFERS_BUILD_FLATLIB",
        "FLATBUFFERS_BUILD_GRPCTEST",
        "FLATBUFFERS_BUILD_LEGACY",
        "FLATBUFFERS_BUILD_SHAREDLIB",
        "FLATBUFFERS_BUILD_TESTS",
        "CXXSTDLIB",
        "CRATE_CC_NO_DEFAULTS",
        "SEARCHKERNEL_BUILD_ENV",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    for base in [
        "CC", "CXX", "AR", "RANLIB", "CFLAGS", "CXXFLAGS", "CPPFLAGS", "LDFLAGS",
    ] {
        names.push(format!("{base}_{target}"));
        names.push(format!("{base}_{target_underscore}"));
        names.push(format!("HOST_{base}"));
    }
    names.sort();
    names.dedup();
    names
}

pub fn rerun_paths(target: &str, host: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for tool in [compiler("c", target, host), compiler("cxx", target, host)] {
        paths.push(tool.path);
    }
    for variable in ["AR", "RANLIB", "LD", "CMAKE"] {
        let value = env::var(variable).unwrap_or_else(|_| variable.to_ascii_lowercase());
        paths.push(resolve_program(Path::new(&value)));
    }
    for path in [
        "/etc/os-release",
        "/etc/ld.so.cache",
        "/usr/lib/x86_64-linux-gnu/libc.so.6",
        "/usr/lib/x86_64-linux-gnu/libstdc++.so.6",
        "/lib/x86_64-linux-gnu/libc.so.6",
        "/lib/x86_64-linux-gnu/libstdc++.so.6",
    ] {
        let path = PathBuf::from(path);
        if path.exists() {
            paths.push(path);
        }
    }
    paths
}

pub fn env_value(name: &str) -> String {
    env::var(name).unwrap_or_default()
}

fn add(fields: &mut BTreeMap<String, String>, key: &str, value: &str) {
    fields.insert(key.to_owned(), value.to_owned());
}

fn command_identity(variable: &str, default: &str, args: &[&str]) -> String {
    let requested = env::var(variable).unwrap_or_else(|_| default.into());
    let path = resolve_program(Path::new(&requested));
    format!(
        "path={}\noutput={}",
        path.display(),
        command_output(&path, args)
    )
}

fn command_output(program: &Path, args: &[&str]) -> String {
    match Command::new(program).args(args).output() {
        Ok(output) => format!(
            "status={}\nstdout={}\nstderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
        Err(error) => format!("error={error}"),
    }
}

fn libc_identity() -> String {
    format!(
        "ldd={}\ngetconf={}\nldconfig={}",
        command_output(Path::new("ldd"), &["--version"]),
        command_output(Path::new("getconf"), &["GNU_LIBC_VERSION"]),
        command_output(Path::new("ldconfig"), &["-p"])
    )
}

fn cxx_runtime_identity(cxx: &Path) -> String {
    let mut output = command_output(cxx, &["-print-file-name=libstdc++.so"]);
    output.push_str("\nlibcxx=");
    output.push_str(&command_output(cxx, &["-print-file-name=libc++.dylib"]));
    output
}

fn escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn unescape(value: &str) -> String {
    let mut output = String::new();
    let mut escaped = false;
    for character in value.chars() {
        if escaped {
            output.push(match character {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                other => other,
            });
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else {
            output.push(character);
        }
    }
    output
}
