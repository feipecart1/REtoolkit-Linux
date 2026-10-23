use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Tool {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub website: String,
    pub repository: String,
    pub license: String,
    pub architectures: Vec<String>,
    pub executable: String,
    pub version_args: Vec<String>,
    pub gui: bool,
    pub apt: Option<String>,
    pub dnf: Option<String>,
    pub flatpak: Option<String>,
    #[serde(default)]
    pub user_deb: Option<crate::installers::user_deb::Source>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub file_args: Vec<String>,
    #[serde(default)]
    pub formats: Vec<String>,
}
#[derive(Deserialize)]
struct Manifest {
    schema_version: u32,
    tools: Vec<Tool>,
}
pub fn load() -> Result<Vec<Tool>, String> {
    parse(include_str!("../../manifests/tools/catalog.json"))
}
pub fn parse(data: &str) -> Result<Vec<Tool>, String> {
    let m: Manifest = serde_json::from_str(data).map_err(|e| e.to_string())?;
    if m.schema_version != 1 {
        return Err("Unsupported catalog schema".into());
    }
    let mut ids = std::collections::HashSet::new();
    for t in &m.tools {
        if !valid_id(&t.id)
            || !valid_id(&t.executable)
            || !ids.insert(&t.id)
            || !t.website.starts_with("https://")
            || !t.repository.starts_with("https://")
            || t.apt.as_deref().is_some_and(|p| !valid_id(p))
            || t.dnf.as_deref().is_some_and(|p| !valid_id(p))
        {
            return Err(format!("Invalid catalog entry: {}", t.id));
        }
    }
    Ok(m.tools)
}
pub fn valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() < 128
        && s.as_bytes()[0].is_ascii_alphanumeric()
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._+-".contains(&c))
}
