//! Edition identity follows installed metadata and a relocatable private prefix.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Light,
    Full,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Edition {
    pub schema_version: u32,
    pub edition: Kind,
    pub version: String,
    pub bundled_tools: Vec<String>,
    pub distribution_tools: Vec<String>,
}
impl Edition {
    pub fn light() -> Self {
        Self {
            schema_version: 1,
            edition: Kind::Light,
            version: env!("CARGO_PKG_VERSION").into(),
            bundled_tools: vec![],
            distribution_tools: vec![],
        }
    }
    pub fn label(&self) -> &'static str {
        match self.edition {
            Kind::Light => "Light",
            Kind::Full => "Full",
        }
    }
    pub fn full(&self) -> bool {
        matches!(self.edition, Kind::Full)
    }
}
pub fn share_for(executable: &Path) -> Option<PathBuf> {
    Some(executable.parent()?.parent()?.join("share/retoolkit-linux"))
}
pub fn share() -> Option<PathBuf> {
    share_for(&std::env::current_exe().ok()?)
}
pub fn current() -> Edition {
    share()
        .and_then(|p| std::fs::read(p.join("edition.json")).ok())
        .and_then(|b| serde_json::from_slice::<Edition>(&b).ok())
        .filter(|e| e.schema_version == 1)
        .unwrap_or_else(Edition::light)
}
pub fn payload_for(executable: &Path) -> Option<PathBuf> {
    Some(
        executable
            .parent()?
            .parent()?
            .join("lib/retoolkit-linux/full"),
    )
}
pub fn payload() -> Option<PathBuf> {
    payload_for(&std::env::current_exe().ok()?)
}
pub fn bundled_executable(tool: &crate::catalog::Tool) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let path = payload()?.join("bin").join(&tool.executable);
    (path.is_file()
        && path
            .metadata()
            .is_ok_and(|m| m.permissions().mode() & 0o111 != 0))
    .then_some(path)
}
pub fn bundled_version(tool: &crate::catalog::Tool) -> Option<String> {
    bundled_executable(tool)?;
    let data: serde_json::Value =
        serde_json::from_slice(&std::fs::read(payload()?.join("sources.json")).ok()?).ok()?;
    let version = data["tools"]
        .as_array()?
        .iter()
        .find(|t| t["id"].as_str() == Some(&tool.id))?["version"]
        .as_str()?;
    Some(format!("{version} (pacote Full)"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edition_prefix_is_relocatable() {
        assert_eq!(
            share_for(Path::new("/tmp/a prefix/usr/bin/retoolkit-linux")),
            Some(PathBuf::from("/tmp/a prefix/usr/share/retoolkit-linux"))
        );
    }
    #[test]
    fn architecture_dependent_payload_uses_lib() {
        assert_eq!(
            payload_for(Path::new("/tmp/portable/usr/bin/retoolkit-linux")),
            Some(PathBuf::from("/tmp/portable/usr/lib/retoolkit-linux/full"))
        );
    }
    #[test]
    fn light_has_no_bundled_tools() {
        let e = Edition::light();
        assert_eq!(e.label(), "Light");
        assert!(!e.full());
        assert!(e.bundled_tools.is_empty());
    }
    #[test]
    fn release_profiles_match_catalog_and_pinned_sources() {
        let profiles: serde_json::Value =
            serde_json::from_str(include_str!("../../packaging/editions/profiles.json")).unwrap();
        let sources: serde_json::Value =
            serde_json::from_str(include_str!("../../packaging/editions/full-sources.json"))
                .unwrap();
        let tools = crate::catalog::load().unwrap();
        for id in profiles["full"]["bundled_tools"].as_array().unwrap() {
            let id = id.as_str().unwrap();
            assert!(tools.iter().any(|t| t.id == id));
            assert!(sources["tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s["id"] == id));
        }
        assert!(profiles["light"]["bundled_tools"]
            .as_array()
            .unwrap()
            .is_empty());
    }
}
