//! Relocatable, generation-specific locations shared by producer and Cargo consumer.
use std::path::{Path, PathBuf};

pub fn runtime_directory(identity: &str) -> Result<PathBuf, String> {
    if identity.len() != 72
        || !identity.starts_with("skrt-v1-")
        || !identity[8..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("invalid immutable consumer runtime identity".into());
    }
    Ok(Path::new(".searchkernel-native").join(identity))
}

pub fn adapter_soname(identity: &str) -> Result<String, String> {
    Ok(format!(
        "$ORIGIN/{}/libsearchkernel_vespa.so.2",
        runtime_directory(identity)?.display()
    ))
}

pub fn abi_probe_source(expected: u32) -> String {
    format!("#include <stdint.h>\nextern uint32_t sk_native_abi_version(void);\nint main(void) {{ return sk_native_abi_version() == {expected}U ? 0 : 1; }}\n")
}

pub fn producer_directory() -> Result<PathBuf, String> {
    match std::env::var("SEARCHKERNEL_RUNTIME_BUILD_ID") {
        Ok(identity) => runtime_directory(&identity),
        Err(std::env::VarError::NotPresent) => Ok(PathBuf::from(".searchkernel-native")),
        Err(error) => Err(format!("invalid consumer runtime build identity: {error}")),
    }
}

/// Check the loader identity without requiring binutils in a Cargo consumer.
/// This reader is deliberately limited to the sole qualified ELF64 LE target.
pub fn verify_adapter_soname(path: &Path, identity: &str) -> Result<(), String> {
    let data = std::fs::read(path).map_err(|error| error.to_string())?;
    let actual = elf_soname(&data)?;
    if actual != adapter_soname(identity)? {
        return Err(
            "identity-mismatch: adapter SONAME does not bind the package runtime identity".into(),
        );
    }
    Ok(())
}

pub fn elf_soname(data: &[u8]) -> Result<&str, String> {
    fn bytes(data: &[u8], offset: u64, size: u64) -> Result<&[u8], String> {
        let end = offset.checked_add(size).ok_or("ELF offset overflow")?;
        let start = usize::try_from(offset).map_err(|_| "ELF offset overflow")?;
        let end = usize::try_from(end).map_err(|_| "ELF offset overflow")?;
        data.get(start..end)
            .ok_or_else(|| "truncated runtime ELF".into())
    }
    fn word(data: &[u8], offset: u64, size: u64) -> Result<u64, String> {
        let mut value = [0u8; 8];
        value[..size as usize].copy_from_slice(bytes(data, offset, size)?);
        Ok(u64::from_le_bytes(value))
    }
    if data.get(..6) != Some(&b"\x7fELF\x02\x01"[..]) {
        return Err("runtime adapter is not a qualified ELF64 little-endian library".into());
    }
    let phoff = word(data, 32, 8)?;
    let phsize = word(data, 54, 2)?;
    let phnum = word(data, 56, 2)?;
    if phsize != 56 || phnum == 0 {
        return Err("invalid ELF program headers".into());
    }
    let headers = bytes(data, phoff, phsize * phnum)?;
    let mut loads = Vec::new();
    let mut dynamic = None;
    for header in headers.chunks_exact(56) {
        let kind = word(header, 0, 4)?;
        let offset = word(header, 8, 8)?;
        let address = word(header, 16, 8)?;
        let size = word(header, 32, 8)?;
        if kind == 1 {
            loads.push((offset, address, size));
        }
        if kind == 2 {
            if dynamic.is_some() {
                return Err("multiple ELF dynamic segments".into());
            }
            dynamic = Some(bytes(data, offset, size)?);
        }
    }
    let mut strtab = None;
    let mut strsize = None;
    let mut soname = None;
    for entry in dynamic
        .ok_or("missing ELF dynamic segment")?
        .chunks_exact(16)
    {
        let tag = word(entry, 0, 8)?;
        let value = word(entry, 8, 8)?;
        match tag {
            0 => break,
            5 => strtab = Some(value),
            10 => strsize = Some(value),
            14 => soname = Some(value),
            _ => {}
        }
    }
    let address = strtab.ok_or("missing ELF string table")?;
    let size = strsize.ok_or("missing ELF string table size")?;
    let index = soname.ok_or("missing ELF SONAME")?;
    if index >= size {
        return Err("ELF SONAME outside string table".into());
    }
    for (offset, base, length) in loads {
        if let Some(relative) = address.checked_sub(base) {
            if relative <= length && size <= length - relative {
                let start = offset
                    .checked_add(relative)
                    .and_then(|start| start.checked_add(index))
                    .ok_or("ELF offset overflow")?;
                let name = bytes(data, start, size - index)?;
                let end = name
                    .iter()
                    .position(|byte| *byte == 0)
                    .ok_or("unterminated ELF SONAME")?;
                return std::str::from_utf8(&name[..end])
                    .map_err(|_| "invalid ELF SONAME encoding".into());
            }
        }
    }
    Err("ELF string table is outside loaded segments".into())
}
