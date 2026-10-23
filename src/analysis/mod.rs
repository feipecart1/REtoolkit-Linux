use object::{Object, ObjectSection};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{fs::OpenOptions, io::Read, path::Path};
pub const MAX_BYTES: u64 = 64 * 1024 * 1024;
#[derive(Debug, Serialize)]
pub struct Report {
    pub path: String,
    pub size: u64,
    pub sha256: String,
    pub format: String,
    pub architecture: String,
    pub entry: Option<u64>,
    pub sections: Vec<String>,
    pub strings: Vec<String>,
    pub warnings: Vec<String>,
}
pub fn inspect(path: &Path) -> Result<Report, String> {
    use std::os::unix::fs::OpenOptionsExt;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| e.to_string())?;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err("Only regular files can be inspected".into());
    }
    if meta.len() > MAX_BYTES {
        return Err("File exceeds the 64 MiB static analysis limit".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("File grew past analysis limit".into());
    }
    let format = identify(&bytes).to_string();
    let mut r = Report {
        path: path.display().to_string(),
        size: bytes.len() as u64,
        sha256: hex::encode(Sha256::digest(&bytes)),
        format,
        architecture: "Unknown".into(),
        entry: None,
        sections: Vec::new(),
        strings: Vec::new(),
        warnings: vec![],
    };
    if r.format == "ELF" || r.format == "PE" {
        match object::File::parse(bytes.as_slice()) {
            Ok(obj) => {
                r.architecture = format!(
                    "{:?} / {}-bit / {} endian",
                    obj.architecture(),
                    if obj.is_64() { 64 } else { 32 },
                    if obj.is_little_endian() {
                        "little"
                    } else {
                        "big"
                    }
                );
                r.entry = Some(obj.entry());
                r.sections = obj
                    .sections()
                    .take(512)
                    .map(|s| {
                        format!(
                            "{}  address={:#x} size={} offset={:?}",
                            s.name().unwrap_or("<invalid name>"),
                            s.address(),
                            s.size(),
                            s.file_range()
                        )
                    })
                    .collect();
            }
            Err(e) => r.warnings.push(format!("Malformed binary: {e}")),
        }
    } else if r.format == "ZIP / possible APK" {
        match zip::ZipArchive::new(std::io::Cursor::new(&bytes)) {
            Ok(mut archive) => {
                let apk = archive
                    .file_names()
                    .filter_map(Result::ok)
                    .any(|n| n == "AndroidManifest.xml");
                if apk {
                    r.format = "APK".into();
                }
                r.warnings.push("Archive metadata only; no content is decompressed or extracted. APK manifest decoding is not implemented.".into());
                for index in 0..archive.len().min(512) {
                    match archive.by_index_raw(index) {
                        Ok(file) => r.sections.push(format!(
                            "{} compressed={} uncompressed={} crc32={:08x}",
                            file.name()
                                .map(|s| s.into_owned())
                                .unwrap_or_else(|_| "<invalid name>".into()),
                            file.compressed_size(),
                            file.size(),
                            file.crc32()
                        )),
                        Err(error) => r.warnings.push(error.to_string()),
                    }
                }
            }
            Err(error) => r.warnings.push(format!("Malformed ZIP: {error}")),
        }
    } else if r.format == "DEX" && bytes.len() >= 112 {
        let u = |n| u32::from_le_bytes(bytes[n..n + 4].try_into().unwrap_or([0; 4]));
        r.sections.push(format!(
            "DEX header: file_size={} header_size={} endian_tag={:#x} string_ids={} class_defs={}",
            u(32),
            u(36),
            u(40),
            u(56),
            u(96)
        ));
        if u(32) as usize != bytes.len() || u(36) != 112 {
            r.warnings.push("Invalid DEX header sizes".into());
        }
    } else {
        r.warnings.push("Format identification only; archive extraction and firmware architecture detection are not implemented. ZIP may contain APK content.".into());
    }
    let mut current = Vec::new();
    for &b in &bytes {
        if b.is_ascii_graphic() || b == b' ' {
            if current.len() < 1024 {
                current.push(b);
            }
        } else {
            if current.len() >= 5 {
                r.strings.push(String::from_utf8_lossy(&current).into());
            }
            current.clear();
        }
        if r.strings.len() >= 2000 {
            break;
        }
    }
    if current.len() >= 5 && r.strings.len() < 2000 {
        r.strings.push(String::from_utf8_lossy(&current).into());
    }
    Ok(r)
}
pub fn identify(b: &[u8]) -> &'static str {
    if b.starts_with(b"\x7fELF") {
        "ELF"
    } else if b.starts_with(b"MZ") {
        "PE"
    } else if b.starts_with(b"dex\n") {
        "DEX"
    } else if b.starts_with(b"PK\x03\x04") {
        "ZIP / possible APK"
    } else if b.starts_with(b"\x1f\x8b") {
        "gzip"
    } else if b.starts_with(b"hsqs") || b.starts_with(b"sqsh") {
        "SquashFS firmware"
    } else if b.starts_with(b"\xfd7zXZ\0") {
        "XZ"
    } else {
        "Unknown / raw firmware"
    }
}
