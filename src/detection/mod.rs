use crate::{
    catalog::Tool,
    process::{self, Spec},
};
use serde::Serialize;
use std::{collections::BTreeMap, fs};
#[derive(Clone, Debug, Serialize)]
pub struct Detected {
    pub id: String,
    pub path: Option<String>,
    pub package_installed: bool,
    pub available: bool,
    pub version: Option<String>,
}
pub fn tool(t: &Tool) -> Detected {
    let path = process::tool_executable(t);
    // Version checks are opt-in catalog commands; no suspect files are executed.
    let version = path.as_ref().and_then(|p| {
        if t.version_args.is_empty() {
            return None;
        }
        process::query(&Spec {
            program: p.clone(),
            args: t.version_args.clone(),
        })
        .ok()
        .filter(|o| o.success)
        .map(|o| {
            o.text
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(200)
                .collect()
        })
    });
    let version = version.or_else(|| crate::edition::bundled_version(t));
    if path.is_none() {
        if let Some(version) = process::flatpak_installed(t) {
            return Detected {
                id: t.id.clone(),
                package_installed: false,
                available: false,
                path: Some(format!(
                    "Flatpak (user): {}",
                    t.flatpak.as_deref().unwrap_or_default()
                )),
                version: Some(version),
            };
        }
    }
    Detected {
        id: t.id.clone(),
        package_installed: false,
        available: false,
        path: path.map(|p| p.display().to_string()),
        version,
    }
}
pub fn all(tools: &[Tool]) -> Vec<Detected> {
    tools.iter().map(tool).collect()
}
pub fn os_release(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.into(), v.trim_matches('"').trim_matches('\'').into()))
        .collect()
}
#[derive(Clone, Debug, Serialize)]
pub struct System {
    pub distribution: String,
    pub id: String,
    pub kernel: String,
    pub architecture: String,
    pub dependencies: BTreeMap<String, bool>,
}
pub fn system() -> System {
    let os = os_release(&fs::read_to_string("/etc/os-release").unwrap_or_default());
    System {
        distribution: os
            .get("PRETTY_NAME")
            .cloned()
            .unwrap_or("Unknown Linux".into()),
        id: os.get("ID").cloned().unwrap_or_default(),
        kernel: fs::read_to_string("/proc/sys/kernel/osrelease")
            .unwrap_or_default()
            .trim()
            .into(),
        architecture: std::env::consts::ARCH.into(),
        dependencies: [
            "pkexec",
            "apt-get",
            "dnf",
            "flatpak",
            "gnome-terminal",
            "python3",
            "java",
        ]
        .iter()
        .map(|s| {
            (
                s.to_string(),
                process::find(s).is_some()
                    || (*s == "java"
                        && crate::edition::payload()
                            .is_some_and(|p| p.join("tools/jdk/bin/java").is_file())),
            )
        })
        .collect(),
    }
}
