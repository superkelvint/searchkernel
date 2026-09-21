#[cfg(feature = "vendored")]
mod build_support;

#[cfg(feature = "vendored")]
mod vendored {
    use std::{
        fs::File,
        io::{BufReader, Read},
        path::{Path, PathBuf},
        process::Command,
    };

    use flate2::read::GzDecoder;
    use ring::digest::{Context, SHA256};
    use tar::Archive;

    use super::build_support::{
        self, fingerprint_path, invalidate_native_output, native_output_exists,
        NativeBuildFingerprint, FLATBUFFERS_SOURCE_SHA256, FLATBUFFERS_SOURCE_URL,
        FLATBUFFERS_VERSION,
    };

    const SOURCE_DIRECTORY_PREFIX: &str = "flatbuffers-25.12.19";

    pub fn build() -> Result<(), String> {
        let target = std::env::var("TARGET").map_err(|error| error.to_string())?;
        let host = std::env::var("HOST").map_err(|error| error.to_string())?;
        for name in build_support::relevant_environment_names(&target) {
            println!("cargo:rerun-if-env-changed={name}");
        }
        for path in build_support::rerun_paths(&target, &host) {
            println!("cargo:rerun-if-changed={}", path.display());
        }

        let out_dir = PathBuf::from(
            std::env::var_os("OUT_DIR").ok_or_else(|| "OUT_DIR is not set".to_owned())?,
        );
        let fingerprint = NativeBuildFingerprint::current();
        let saved_path = fingerprint_path(&out_dir);
        let saved = NativeBuildFingerprint::read(&saved_path);
        let rebuild = !native_output_exists(&out_dir) || saved.as_ref() != Some(&fingerprint);
        if let Some(previous) = saved.as_ref() {
            if previous != &fingerprint {
                warn_changed(&fingerprint, previous);
            }
        } else if native_output_exists(&out_dir) {
            println!(
                "cargo:warning=searchkernel: native FlatBuffers provenance is missing; rebuilding cached artifacts"
            );
        }

        if rebuild {
            invalidate_native_output(&out_dir)
                .map_err(|error| format!("invalidate FlatBuffers native output: {error}"))?;
            build_vendored(&out_dir)?;
        }

        match probe(&out_dir, &target, &host) {
            Ok(()) => {}
            Err(first_error) => {
                println!(
                    "cargo:warning=searchkernel: FlatBuffers native link probe failed; rebuilding the affected native component once"
                );
                invalidate_native_output(&out_dir)
                    .map_err(|error| format!("invalidate after probe failure: {error}"))?;
                build_vendored(&out_dir)?;
                if let Err(second_error) = probe(&out_dir, &target, &host) {
                    return Err(format!(
                        "FlatBuffers native compatibility probe failed after one automatic clean rebuild\n\
                         component=flatbuffers {FLATBUFFERS_VERSION}\n\
                         compiler={}\n\
                         target={target}\n\
                         first probe={first_error}\n\
                         second probe={second_error}\n",
                        fingerprint
                            .fields
                            .get("cxx.compiler")
                            .map(String::as_str)
                            .unwrap_or("unknown")
                    ));
                }
            }
        }

        fingerprint
            .write(&saved_path)
            .map_err(|error| format!("persist FlatBuffers native fingerprint: {error}"))?;
        println!("cargo:root={}", out_dir.display());
        println!(
            "cargo:rustc-env=FLATC_PATH={}",
            out_dir.join("bin/flatc").display()
        );
        Ok(())
    }

    fn warn_changed(current: &NativeBuildFingerprint, previous: &NativeBuildFingerprint) {
        let changed = current.changed_fields(previous);
        let summary = if changed.is_empty() {
            "native build record is invalid".to_owned()
        } else {
            changed.join(", ")
        };
        println!(
            "cargo:warning=searchkernel: native FlatBuffers build environment changed; rebuilding cached native artifacts (changed: {summary})"
        );
    }

    fn build_vendored(out_dir: &Path) -> Result<(), String> {
        let temporary = tempfile::tempdir().map_err(|error| error.to_string())?;
        let archive = temporary.path().join("flatbuffers.tar.gz");
        let mut file = File::create(&archive).map_err(|error| error.to_string())?;
        let mut response = reqwest::blocking::get(FLATBUFFERS_SOURCE_URL)
            .map_err(|error| format!("download FlatBuffers {FLATBUFFERS_VERSION}: {error}"))?;
        response
            .copy_to(&mut file)
            .map_err(|error| format!("download FlatBuffers {FLATBUFFERS_VERSION}: {error}"))?;
        checksum_check(&archive)?;

        let extraction = temporary.path().join("flatbuffers");
        let tar_gz = File::open(&archive).map_err(|error| error.to_string())?;
        Archive::new(GzDecoder::new(tar_gz))
            .unpack(&extraction)
            .map_err(|error| format!("extract FlatBuffers {FLATBUFFERS_VERSION}: {error}"))?;
        let source = extraction.join(SOURCE_DIRECTORY_PREFIX);
        let built = cmake::Config::new(source).out_dir(out_dir).build();
        if !native_output_exists(&built) {
            return Err(format!(
                "FlatBuffers CMake build completed without expected outputs under {}",
                built.display()
            ));
        }
        Ok(())
    }

    fn checksum_check(path: &Path) -> Result<(), String> {
        let mut context = Context::new(&SHA256);
        let file = File::open(path).map_err(|error| error.to_string())?;
        let mut reader = BufReader::new(file);
        let mut buffer = [0_u8; 16 * 1024];
        loop {
            let count = reader
                .read(&mut buffer)
                .map_err(|error| error.to_string())?;
            if count == 0 {
                break;
            }
            context.update(&buffer[..count]);
        }
        let digest = context.finish();
        let actual = digest
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        if actual == FLATBUFFERS_SOURCE_SHA256 {
            Ok(())
        } else {
            Err(format!(
                "FlatBuffers source checksum mismatch: expected {FLATBUFFERS_SOURCE_SHA256}, got {actual}"
            ))
        }
    }

    fn probe(out_dir: &Path, target: &str, host: &str) -> Result<(), String> {
        let probe_dir = out_dir.join("probe");
        std::fs::create_dir_all(&probe_dir).map_err(|error| error.to_string())?;
        let source = probe_dir.join("flatbuffers_link_probe.cc");
        let object = probe_dir.join("flatbuffers_link_probe.o");
        let executable = probe_dir.join("flatbuffers_link_probe");
        std::fs::write(
            &source,
            "#include <flatbuffers/idl.h>\n\
             int main() { flatbuffers::Parser parser; return parser.Parse(\
             \"table X { value:long; } root_type X;\") ? 0 : 1; }\n",
        )
        .map_err(|error| error.to_string())?;

        let mut compiler_config = cc::Build::new();
        compiler_config
            .cpp(true)
            .cargo_metadata(false)
            .target(target)
            .host(host);
        let compiler = compiler_config.get_compiler();
        let mut compile = compiler.to_command();
        compile
            .arg("-std=c++17")
            .arg("-I")
            .arg(out_dir.join("include"))
            .args(split_flags("CPPFLAGS"))
            .arg("-c")
            .arg(&source)
            .arg("-o")
            .arg(&object);
        run_command(&mut compile, "compile FlatBuffers compatibility probe")?;

        let mut link = compiler.to_command();
        link.arg(&object)
            .arg("-L")
            .arg(out_dir.join("lib"))
            .arg("-lflatbuffers")
            .args(split_flags("LDFLAGS"));
        let standard_library = std::env::var("CXXSTDLIB").unwrap_or_else(|_| {
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
        run_command(&mut link, "link FlatBuffers compatibility probe")?;
        let status = Command::new(&executable)
            .status()
            .map_err(|error| format!("run FlatBuffers compatibility probe: {error}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("probe executable exited with {status}"))
        }
    }

    fn split_flags(variable: &str) -> Vec<String> {
        std::env::var(variable)
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_owned)
            .collect()
    }

    fn run_command(command: &mut Command, description: &str) -> Result<(), String> {
        let output = command
            .output()
            .map_err(|error| format!("{description}: {error}"))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(format!(
                "{description} failed with {}\nstdout={}\nstderr={}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }
}

fn main() {
    #[cfg(feature = "vendored")]
    if let Err(error) = vendored::build() {
        panic!("{error}");
    }
}
