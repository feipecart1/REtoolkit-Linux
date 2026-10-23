use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
#[derive(Clone, Debug)]
pub struct Paths {
    pub config: PathBuf,
    pub data: PathBuf,
    pub cache: PathBuf,
}
fn xdg(key: &str, suffix: &str) -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os(key)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
    {
        return Ok(path.join("retoolkit-linux"));
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .map(|p| p.join(suffix).join("retoolkit-linux"))
        .ok_or("No absolute HOME or XDG directory".into())
}
impl Paths {
    pub fn discover() -> Result<Self, String> {
        Ok(Self {
            config: xdg("XDG_CONFIG_HOME", ".config")?,
            data: xdg("XDG_DATA_HOME", ".local/share")?,
            cache: xdg("XDG_CACHE_HOME", ".cache")?,
        })
    }
    pub fn create(&self) -> Result<(), String> {
        use std::os::unix::fs::DirBuilderExt;
        for p in [&self.config, &self.data, &self.cache] {
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(p)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Settings {
    pub follow_system_theme: bool,
    pub check_updates: bool,
}
impl Settings {
    pub fn load(paths: &Paths) -> Result<Self, String> {
        match fs::read(paths.config.join("settings.json")) {
            Ok(b) => serde_json::from_slice(&b).map_err(|e| e.to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.to_string()),
        }
    }
    pub fn save(&self, paths: &Paths) -> Result<(), String> {
        paths.create()?;
        let p = paths.config.join("settings.json.tmp");
        fs::write(
            &p,
            serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        fs::rename(p, paths.config.join("settings.json")).map_err(|e| e.to_string())
    }
}
pub fn history(paths: &Paths) -> String {
    fs::read_to_string(paths.data.join("history.log"))
        .unwrap_or_default()
        .lines()
        .rev()
        .take(20)
        .collect::<Vec<_>>()
        .join("\n")
}
pub fn record(paths: &Paths, text: &str) -> Result<(), String> {
    paths.create()?;
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    crate::process::save_log(
        &paths.data.join("history.log"),
        &format!("{timestamp} {}", text.replace(['\n', '\r'], " ")),
    )
}
