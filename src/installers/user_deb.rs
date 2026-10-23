//! Pinned distribution archive installed in a private user prefix. No maintainer
//! scripts are executed; only explicitly allowlisted Ubuntu package artifacts.
use super::{Manager, Plan};
use crate::{
    catalog::Tool,
    config::Paths,
    detection,
    process::{self, Spec},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{path::PathBuf, sync::atomic::AtomicBool, time::Duration};
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Source {
    pub url: String,
    pub sha256: String,
    pub version: String,
    pub distribution: String,
    pub version_id: String,
    pub packages: Vec<String>,
}
fn validate(source: &Source) -> Result<(), String> {
    if !source
        .url
        .starts_with("https://archive.ubuntu.com/ubuntu/pool/")
        || source.url.contains(['?', '#'])
        || source.sha256.len() != 64
        || hex::decode(&source.sha256).is_err()
    {
        return Err("Invalid pinned Ubuntu artifact".into());
    }
    Ok(())
}
pub fn plan(tool: &Tool) -> Result<Plan, String> {
    let source = tool.user_deb.clone().ok_or("No pinned user archive")?;
    validate(&source)?;
    let os = detection::os_release(
        &std::fs::read_to_string("/etc/os-release").map_err(|e| e.to_string())?,
    );
    if os.get("ID") != Some(&source.distribution)
        || os.get("VERSION_ID") != Some(&source.version_id)
        || std::env::consts::ARCH != "x86_64"
    {
        return Err("This pinned user package is validated only for Ubuntu 24.04 x86_64".into());
    }
    for package in &source.packages {
        if !crate::catalog::valid_id(package) {
            return Err("Invalid dependency".into());
        }
        let check = process::query(&Spec::new(
            process::find("dpkg-query").ok_or("dpkg-query missing")?,
            &["-W", "-f=${db:Status-Abbrev}", package],
        ))?;
        if !check.success || !check.text.starts_with("ii ") {
            return Err(format!("Required system dependency missing: {package}"));
        }
    }
    let paths = Paths::discover()?;
    let destination = paths.data.join("tools").join(&tool.id);
    if destination.exists() {
        return Err("User tool installation already exists; refusing to overwrite".into());
    }
    Ok(Plan{tool_id:tool.id.clone(),manager:Manager::UserDeb,package:format!("{} {}",tool.id,source.version),source:source.url.clone(),command:Spec::new(process::find("dpkg-deb").ok_or("dpkg-deb missing")?,&["--extract","<verified-download.deb>",destination.to_str().ok_or("Invalid user path")?]),preview:format!("SHA-256: {}\nDestino: {}\nDependências já instaladas: {:?}\nSem root, scripts de mantenedor ou alteração do banco APT. Remova esta pasta para desinstalar esta cópia privada.",source.sha256,destination.display(),source.packages),cancellable:true,artifact:Some(source)})
}
pub fn install(plan: &Plan, cancel: &AtomicBool) -> Result<String, String> {
    install_observed(plan, cancel, None)
}
pub fn install_observed(
    plan: &Plan,
    cancel: &AtomicBool,
    observer: Option<process::Observer>,
) -> Result<String, String> {
    let source = plan.artifact.as_ref().ok_or("Missing artifact")?;
    validate(source)?;
    if !crate::catalog::valid_id(&plan.tool_id) {
        return Err("Invalid tool ID".into());
    }
    let paths = Paths::discover()?;
    paths.create()?;
    let tools = paths.data.join("tools");
    std::fs::create_dir_all(&tools).map_err(|e| e.to_string())?;
    let destination = tools.join(&plan.tool_id);
    if destination.exists() {
        return Err("Destination exists; files preserved".into());
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let staging = tools.join(format!(".{}-{stamp}", plan.tool_id));
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&staging)
        .map_err(|e| e.to_string())?;
    let archive = staging.join("artifact.deb");
    let unpack = staging.join("unpacked");
    let result = (|| {
        if let Some(o) = &observer {
            o("Downloading pinned official Ubuntu package…\n");
        }
        let curl = process::find("curl").ok_or("curl missing")?;
        let out = process::run_observed(
            &Spec::new(
                curl,
                &[
                    "--fail",
                    "--proto",
                    "=https",
                    "--tlsv1.2",
                    "--max-time",
                    "120",
                    "--max-filesize",
                    "67108864",
                    "--output",
                    archive.to_str().ok_or("Invalid cache path")?,
                    &source.url,
                ],
            ),
            Duration::from_secs(130),
            cancel,
            observer.clone(),
        )?;
        if !out.success {
            return Err(out.text);
        }
        let bytes = std::fs::read(&archive).map_err(|e| e.to_string())?;
        if hex::encode(Sha256::digest(&bytes)) != source.sha256 {
            return Err("SHA-256 verification failed; artifact rejected".into());
        }
        if let Some(o) = &observer {
            o("SHA-256 verified. Extracting into private prefix…\n");
        }
        let out = process::run(
            &Spec::new(
                process::find("dpkg-deb").ok_or("dpkg-deb missing")?,
                &[
                    "--extract",
                    archive.to_str().ok_or("Invalid archive path")?,
                    unpack.to_str().ok_or("Invalid prefix")?,
                ],
            ),
            Duration::from_secs(30),
            cancel,
        )?;
        if !out.success {
            return Err(out.text);
        }
        // Only a known standalone tool is enabled; this is not a generic DEB installer.
        if plan.tool_id != "hashdeep" {
            return Err("Tool is not allowlisted for private DEB installation".into());
        }
        let executable = unpack.join("usr/bin/hashdeep");
        let binary = std::fs::read(&executable).map_err(|e| e.to_string())?;
        if !binary.starts_with(b"\x7fELF") {
            return Err("Installed executable is not ELF".into());
        }
        std::fs::write(
            unpack.join("retoolkit-install.json"),
            serde_json::to_vec_pretty(source).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        // Destination is rechecked immediately before promotion; preserve existing data.
        if destination.exists() {
            return Err("Destination appeared during installation".into());
        }
        use std::os::unix::ffi::OsStrExt;
        let from =
            std::ffi::CString::new(unpack.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
        let to = std::ffi::CString::new(destination.as_os_str().as_bytes())
            .map_err(|e| e.to_string())?;
        // Linux atomic promotion: even an empty directory created concurrently is preserved.
        if unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                from.as_ptr(),
                libc::AT_FDCWD,
                to.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        } != 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let version = process::query(&Spec::new(destination.join("usr/bin/hashdeep"), &["-V"]))?;
        if !version.success {
            return Err(format!(
                "Installed but version validation failed: {}",
                version.text
            ));
        }
        Ok(format!(
            "Downloaded {}\nSHA-256 verified: {}\nInstalled at {}\nVersion validation: {}",
            source.url,
            source.sha256,
            destination.display(),
            version.text
        ))
    })();
    let _ = std::fs::remove_dir_all(staging);
    result
}
pub fn installed_path(tool: &Tool) -> Option<PathBuf> {
    crate::process::tool_executable(tool)
}
