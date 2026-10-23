use retoolkit_linux::{
    analysis, catalog,
    config::{Paths, Settings},
    detection, installers,
    process::{self, Spec},
};
use std::{path::PathBuf, sync::atomic::AtomicBool, time::Duration};
fn temp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("retoolkit-test-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&p).unwrap();
    p
}
#[test]
fn catalog_integrity() {
    let tools = catalog::load().unwrap();
    assert_eq!(tools.len(), 36);
    assert!(tools.iter().any(|t| t.id == "ghidra" && t.apt.is_none()));
    assert!(tools
        .iter()
        .any(|t| t.id == "binaryninja" && t.license == "Proprietary"));
    assert!(catalog::parse("{\"schema_version\":99,\"tools\":[]}").is_err());
}
#[test]
fn catalog_rejects_command_injection() {
    let original = include_str!("../manifests/tools/catalog.json");
    let bad = original.replacen(
        "\"executable\": \"ghidra\"",
        "\"executable\": \"; touch /tmp/owned\"",
        1,
    );
    assert!(catalog::parse(&bad).is_err());
    for s in ["", "-option", "/bin/sh", "a b", "$(id)", "a;id", "a\nb"] {
        assert!(!catalog::valid_id(s));
    }
}
#[test]
fn elf_inspection_matches_system_hash() {
    let path = std::env::current_exe().unwrap();
    let r = analysis::inspect(&path).unwrap();
    assert_eq!(r.format, "ELF");
    assert!(r.architecture.contains("X86_64"));
    assert!(!r.sections.is_empty());
    let expected =
        process::query(&Spec::new("/usr/bin/sha256sum", &[path.to_str().unwrap()])).unwrap();
    assert_eq!(r.sha256, expected.text.split_whitespace().next().unwrap());
}
#[test]
fn malformed_binaries_are_reported() {
    let dir = temp("malformed");
    for (n, bytes) in [
        ("elf", b"\x7fELFbad".as_slice()),
        ("pe", b"MZbad".as_slice()),
    ] {
        let p = dir.join(n);
        std::fs::write(&p, bytes).unwrap();
        let r = analysis::inspect(&p).unwrap();
        assert!(!r.warnings.is_empty());
    }
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn rejects_oversized_and_nonregular() {
    let dir = temp("oversize");
    let p = dir.join("large");
    std::fs::File::create(&p)
        .unwrap()
        .set_len(analysis::MAX_BYTES + 1)
        .unwrap();
    assert!(analysis::inspect(&p).is_err());
    assert!(analysis::inspect(&dir).is_err());
    assert!(analysis::inspect(std::path::Path::new("/dev/zero")).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn unknown_data_is_never_executed() {
    let dir = temp("unknown");
    let p = dir.join("untrusted");
    std::fs::write(&p, b"#!/bin/sh\ntouch /tmp/retoolkit-must-never-exist\n").unwrap();
    let r = analysis::inspect(&p).unwrap();
    assert_eq!(r.format, "Unknown / raw firmware");
    assert_eq!(r.size, 48);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn detects_archive_and_dex() {
    assert_eq!(analysis::identify(b"dex\n035\0"), "DEX");
    assert_eq!(analysis::identify(b"PK\x03\x04"), "ZIP / possible APK");
    assert_eq!(analysis::identify(b"hsqs"), "SquashFS firmware");
}
#[test]
fn process_arguments_are_literal() {
    let o = process::query(&Spec::new(
        "/usr/bin/printf",
        &["%s", "$(id); test with spaces"],
    ))
    .unwrap();
    assert!(o.success);
    assert_eq!(o.text, "$(id); test with spaces");
}
#[test]
fn process_timeout_and_failure() {
    let now = std::time::Instant::now();
    assert!(process::run(
        &Spec::new("/usr/bin/sleep", &["20"]),
        Duration::from_millis(80),
        &AtomicBool::new(false)
    )
    .is_err());
    assert!(now.elapsed() < Duration::from_secs(3));
    assert!(
        !process::query(&Spec::new("/usr/bin/false", &[]))
            .unwrap()
            .success
    );
    assert!(process::query(&Spec::new("/not/an/executable", &[])).is_err());
}
#[test]
fn configuration_roundtrip_and_invalid_json() {
    let dir = temp("config");
    let paths = Paths {
        config: dir.join("config"),
        data: dir.join("data"),
        cache: dir.join("cache"),
    };
    assert!(!Settings::load(&paths).unwrap().follow_system_theme);
    Settings {
        follow_system_theme: true,
        check_updates: false,
    }
    .save(&paths)
    .unwrap();
    assert!(Settings::load(&paths).unwrap().follow_system_theme);
    std::fs::write(paths.config.join("settings.json"), b"broken").unwrap();
    assert!(Settings::load(&paths).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn os_release_parser() {
    let os = detection::os_release("ID=ubuntu\nPRETTY_NAME=\"Ubuntu 24.04 LTS\"\n# ignored\n");
    assert_eq!(os["ID"], "ubuntu");
    assert_eq!(os["PRETTY_NAME"], "Ubuntu 24.04 LTS");
}
#[test]
fn manual_tools_have_no_install_plan() {
    let tools = catalog::load().unwrap();
    let sys = detection::system();
    let t = tools.iter().find(|t| t.id == "binaryninja").unwrap();
    assert!(installers::plan(t, &sys, installers::Action::Install).is_err());
}
#[test]
fn pinned_download_rejects_bad_origin_without_network() {
    let dir = temp("pinned");
    let spec = installers::PinnedBinary {
        url: "http://untrusted.test/tool".into(),
        filename: "tool".into(),
        sha256: "0".repeat(64),
    };
    assert!(installers::install_pinned(&spec, &dir, "example.org").is_err());
    assert!(!dir.join("tool").exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn zip_and_apk_are_inspected_without_extracting() {
    use std::io::{Cursor, Write};
    let dir = temp("zip");
    let cursor = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(cursor);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("AndroidManifest.xml", options).unwrap();
    zip.write_all(b"manifest fixture").unwrap();
    zip.start_file("../../outside", options).unwrap();
    zip.write_all(b"never extract this").unwrap();
    let bytes = zip.finish().unwrap().into_inner();
    let path = dir.join("test.apk");
    std::fs::write(&path, bytes).unwrap();
    let report = analysis::inspect(&path).unwrap();
    assert_eq!(report.format, "APK");
    assert_eq!(report.sections.len(), 2);
    assert!(!dir.join("outside").exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn inherited_output_cannot_bypass_timeout() {
    let script = "import subprocess; subprocess.Popen(['/usr/bin/sleep','15'])";
    let start = std::time::Instant::now();
    let result = process::run(
        &Spec::new("/usr/bin/python3", &["-c", script]),
        Duration::from_millis(300),
        &AtomicBool::new(false),
    );
    assert!(result.is_err());
    assert!(start.elapsed() < Duration::from_secs(3));
}
