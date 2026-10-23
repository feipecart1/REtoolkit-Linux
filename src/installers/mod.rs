pub mod user_deb;
use crate::{
    catalog::Tool,
    detection::System,
    process::{self, Spec},
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::atomic::AtomicBool, time::Duration};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Manager {
    Apt,
    Dnf,
    Flatpak,
    UserDeb,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plan {
    pub tool_id: String,
    pub manager: Manager,
    pub package: String,
    pub source: String,
    pub command: Spec,
    pub preview: String,
    pub cancellable: bool,
    #[serde(default)]
    pub artifact: Option<user_deb::Source>,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Install,
    Update,
    Remove,
}
impl Action {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Install => "Install",
            Self::Update => "Update",
            Self::Remove => "Remove",
        }
    }
}
fn supported_manager(system: &System) -> Option<Manager> {
    if ["ubuntu", "debian", "linuxmint", "pop"].contains(&system.id.as_str())
        && process::find("apt-get").is_some()
    {
        Some(Manager::Apt)
    } else if ["fedora", "rhel", "centos", "rocky", "almalinux"].contains(&system.id.as_str())
        && process::find("dnf").is_some()
    {
        Some(Manager::Dnf)
    } else {
        None
    }
}
pub fn plan(tool: &Tool, system: &System, action: Action) -> Result<Plan, String> {
    if !tool.architectures.contains(&system.architecture) {
        return Err("Unsupported architecture".into());
    }
    let manager = supported_manager(system)
        .ok_or("No supported native package manager for this distribution")?;
    let package = match manager {
        Manager::Apt => tool.apt.clone(),
        Manager::Dnf => tool.dnf.clone(),
        _ => None,
    }
    .ok_or("No verified native package mapping; use official source")?;
    if !crate::catalog::valid_id(&package) {
        return Err("Invalid package name".into());
    }
    let (program, args, preview) = match manager {
        Manager::Apt => {
            let p = process::find("apt-get").ok_or("apt-get missing")?;
            let mut a = vec!["--simulate".into()];
            if action == Action::Update {
                a.push("--only-upgrade".into());
            }
            a.push(
                if action == Action::Remove {
                    "remove"
                } else {
                    "install"
                }
                .into(),
            );
            a.push(package.clone());
            let check = process::query(&Spec {
                program: p.clone(),
                args: a,
            })?;
            if !check.success {
                return Err(check.text);
            }
            let mut a = vec!["-y".into()];
            if action == Action::Update {
                a.push("--only-upgrade".into());
            }
            a.push(
                if action == Action::Remove {
                    "remove"
                } else {
                    "install"
                }
                .into(),
            );
            a.push(package.clone());
            (p, a, check.text)
        }
        Manager::Dnf => {
            let p = process::find("dnf").ok_or("dnf missing")?;
            let check = process::query(&Spec {
                program: p.clone(),
                args: vec!["--cacheonly".into(), "info".into(), package.clone()],
            })?;
            if !check.success {
                return Err(check.text);
            }
            let verb = match action {
                Action::Install => "install",
                Action::Update => "upgrade",
                Action::Remove => "remove",
            };
            (
                p,
                vec![
                    "-y".into(),
                    "--setopt=clean_requirements_on_remove=False".into(),
                    verb.into(),
                    package.clone(),
                ],
                check.text,
            )
        }
        _ => unreachable!(),
    };
    let pkexec = process::find("pkexec")
        .ok_or("polkit pkexec unavailable; install through your system package manager")?;
    let mut elevated_args = vec![program.display().to_string()];
    elevated_args.extend(args);
    Ok(Plan {
        tool_id: tool.id.clone(),
        manager,
        package,
        source:
            "Configured distribution repositories (signature verification by the package manager)"
                .into(),
        command: Spec {
            program: pkexec,
            args: elevated_args,
        },
        preview,
        cancellable: false,
        artifact: None,
    })
}
pub fn flatpak_plan(tool: &Tool, action: Action) -> Result<Plan, String> {
    let id = tool.flatpak.as_ref().ok_or("No Flatpak mapping")?;
    if !crate::catalog::valid_id(id) {
        return Err("Invalid Flatpak application ID".into());
    }
    let program = process::find("flatpak").ok_or("Flatpak unavailable")?;
    let check = if action == Action::Remove {
        process::query(&Spec::new(&program, &["info", "--user", id]))?
    } else {
        process::query(&Spec::new(
            &program,
            &["remote-info", "--user", "flathub", id],
        ))?
    };
    if !check.success {
        return Err(
            "User Flathub remote or app unavailable. No remote will be added automatically.".into(),
        );
    }
    let args = match action {
        Action::Install => vec!["install", "--user", "-y", "flathub", id],
        Action::Update => vec!["update", "--user", "-y", id],
        Action::Remove => vec!["uninstall", "--user", "-y", id],
    }
    .into_iter()
    .map(String::from)
    .collect();
    Ok(Plan {
        tool_id: tool.id.clone(),
        manager: Manager::Flatpak,
        package: id.clone(),
        source: "Existing user Flathub remote; inspect metadata and permissions below".into(),
        command: Spec { program, args },
        preview: check.text,
        cancellable: true,
        artifact: None,
    })
}
pub fn execute(plan: &Plan, cancel: &AtomicBool) -> Result<String, String> {
    execute_observed(plan, cancel, None)
}
pub fn execute_observed(
    plan: &Plan,
    cancel: &AtomicBool,
    observer: Option<process::Observer>,
) -> Result<String, String> {
    if plan.manager == Manager::UserDeb {
        return user_deb::install_observed(plan, cancel, observer);
    }
    let output = process::run_observed(&plan.command, Duration::from_secs(3600), cancel, observer)?;
    if output.success {
        Ok(output.text)
    } else {
        Err(output.text)
    }
}

pub fn native_version(tool: &Tool, sys: &System) -> Option<String> {
    match supported_manager(sys)? {
        Manager::Apt => {
            let pkg = tool.apt.as_ref()?;
            process::query(&Spec::new(
                process::find("dpkg-query")?,
                &["-W", "-f=${db:Status-Abbrev} ${Version}", pkg],
            ))
            .ok()
            .filter(|o| o.success && o.text.starts_with("ii "))
            .map(|o| o.text.trim_start_matches("ii ").trim().to_string())
        }
        Manager::Dnf => {
            let pkg = tool.dnf.as_ref()?;
            process::query(&Spec::new(
                process::find("rpm")?,
                &["-q", "--qf", "%{VERSION}-%{RELEASE}", pkg],
            ))
            .ok()
            .filter(|o| o.success)
            .map(|o| o.text.trim().to_string())
        }
        _ => None,
    }
}

pub fn available_update(tool: &Tool, sys: &System) -> Option<String> {
    if supported_manager(sys) != Some(Manager::Apt) {
        return None;
    }
    let installed = native_version(tool, sys)?;
    let pkg = tool.apt.as_ref()?;
    let policy = process::query(&Spec::new(process::find("apt-cache")?, &["policy", pkg])).ok()?;
    let candidate = policy
        .text
        .lines()
        .find_map(|l| l.trim().strip_prefix("Candidate: "))?;
    let cmp = process::query(&Spec::new(
        process::find("dpkg")?,
        &["--compare-versions", candidate, "gt", &installed],
    ))
    .ok()?;
    cmp.success.then(|| candidate.to_string())
}
// Optional pinned official user binary: no arbitrary URL entry or shell scripts.
#[derive(Debug, Deserialize)]
pub struct PinnedBinary {
    pub url: String,
    pub sha256: String,
    pub filename: String,
}
pub fn install_pinned(
    binary: &PinnedBinary,
    destination: &std::path::Path,
    allowed_host: &str,
) -> Result<PathBuf, String> {
    use sha2::{Digest, Sha256};
    use std::os::unix::fs::PermissionsExt;
    if !binary.url.starts_with(&format!("https://{allowed_host}/"))
        || allowed_host.is_empty()
        || !crate::catalog::valid_id(&binary.filename)
        || binary.sha256.len() != 64
        || hex::decode(&binary.sha256).is_err()
    {
        return Err("Invalid pinned official source or checksum".into());
    }
    std::fs::create_dir_all(destination).map_err(|e| e.to_string())?;
    let target = destination.join(&binary.filename);
    if target.exists() {
        return Err("Refusing to overwrite an existing binary".into());
    }
    let tmp = destination.join(format!(".{}.download", binary.filename));
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        // No redirects: the download must remain on the explicitly approved host.
        let out = process::query(&Spec::new(
            process::find("curl").ok_or("curl missing")?,
            &[
                "--fail",
                "--proto",
                "=https",
                "--tlsv1.2",
                "--max-time",
                "10",
                "--max-filesize",
                "67108864",
                "--output",
                tmp.to_str().ok_or("Invalid path")?,
                &binary.url,
            ],
        ))?;
        if !out.success {
            return Err(out.text);
        }
        let bytes = std::fs::read(&tmp).map_err(|e| e.to_string())?;
        if hex::encode(Sha256::digest(&bytes)) != binary.sha256.to_lowercase() {
            return Err("SHA-256 mismatch".into());
        }
        if !bytes.starts_with(b"\x7fELF") {
            return Err("Pinned binary is not ELF".into());
        }
        use std::io::Write;
        output.flush().map_err(|e| e.to_string())?;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
        // hard_link is atomic and refuses replacement if the target appeared meanwhile.
        std::fs::hard_link(&tmp, &target).map_err(|e| e.to_string())?;
        Ok(target)
    })();
    let _ = std::fs::remove_file(tmp);
    result
}

/// Availability is local-cache evidence, not a promise that a download will succeed.
pub fn available(tool: &Tool, system: &System) -> bool {
    if !tool.architectures.contains(&system.architecture) {
        return false;
    }
    if supported_manager(system) == Some(Manager::Apt) {
        if let (Some(package), Some(program)) = (&tool.apt, process::find("apt-cache")) {
            if let Ok(out) = process::query(&Spec::new(program, &["policy", package])) {
                return out.success
                    && out
                        .text
                        .lines()
                        .filter_map(|l| l.trim().strip_prefix("Candidate: "))
                        .any(|v| v != "(none)");
            }
        }
    }
    if supported_manager(system) == Some(Manager::Dnf) {
        if let (Some(package), Some(program)) = (&tool.dnf, process::find("dnf")) {
            return process::query(&Spec::new(program, &["--cacheonly", "info", package]))
                .is_ok_and(|o| o.success);
        }
    }
    false
}
