use serde::{Deserialize, Serialize};
use std::os::unix::process::CommandExt;
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Spec {
    pub program: PathBuf,
    pub args: Vec<String>,
}
impl Spec {
    pub fn new(program: impl Into<PathBuf>, args: &[&str]) -> Self {
        Self {
            program: program.into(),
            args: args.iter().map(|s| s.to_string()).collect(),
        }
    }
    pub fn display(&self) -> String {
        format!("{} {:?}", self.program.display(), self.args)
    }
}
#[derive(Debug)]
pub struct Output {
    pub success: bool,
    pub text: String,
}
pub type Observer = Arc<dyn Fn(&str) + Send + Sync>;
fn drain(mut reader: impl Read, cap: usize, observer: Option<Observer>) -> Vec<u8> {
    let mut out = Vec::new();
    let mut buf = [0; 8192];
    while let Ok(n) = reader.read(&mut buf) {
        if n == 0 {
            break;
        }
        let keep = n.min(cap.saturating_sub(out.len()));
        out.extend_from_slice(&buf[..keep]);
        if keep > 0 {
            if let Some(observer) = &observer {
                observer(&String::from_utf8_lossy(&buf[..keep]));
            }
        }
    }
    out
}
/// Keep bundled GTK libraries out of system package managers and launched tools.
pub fn external_command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut cmd = Command::new(program);
    if std::env::var_os("RETOOLKIT_APPDIR").is_some() {
        for (name, saved) in [
            ("LD_LIBRARY_PATH", "RETOOLKIT_HOST_LD_LIBRARY_PATH"),
            ("XDG_DATA_DIRS", "RETOOLKIT_HOST_XDG_DATA_DIRS"),
            (
                "GSETTINGS_SCHEMA_DIR",
                "RETOOLKIT_HOST_GSETTINGS_SCHEMA_DIR",
            ),
        ] {
            match std::env::var_os(saved).filter(|s| !s.is_empty()) {
                Some(value) => {
                    cmd.env(name, value);
                }
                None => {
                    cmd.env_remove(name);
                }
            }
        }
        cmd.env_remove("RETOOLKIT_APPDIR");
    }
    cmd
}
pub fn run(spec: &Spec, timeout: Duration, cancel: &AtomicBool) -> Result<Output, String> {
    run_observed(spec, timeout, cancel, None)
}
pub fn run_observed(
    spec: &Spec,
    timeout: Duration,
    cancel: &AtomicBool,
    observer: Option<Observer>,
) -> Result<Output, String> {
    let mut command = if std::env::current_exe().is_ok_and(|p| p == spec.program) {
        Command::new(&spec.program)
    } else {
        external_command(&spec.program)
    };
    let mut child = command
        .args(&spec.args)
        .env("LC_ALL", "C")
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{}: {e}", spec.program.display()))?;
    let stdout = child.stdout.take().ok_or("No stdout")?;
    let stderr = child.stderr.take().ok_or("No stderr")?;
    let second = observer.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let tx_err = tx.clone();
    thread::spawn(move || {
        let _ = tx.send((true, drain(stdout, 4 * 1024 * 1024, observer)));
    });
    thread::spawn(move || {
        let _ = tx_err.send((false, drain(stderr, 4 * 1024 * 1024, second)));
    });
    let start = Instant::now();
    let result = loop {
        if cancel.load(Ordering::Relaxed) || start.elapsed() > timeout {
            // Only the process group created for this child is signalled.
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
            break Err(
                "Operation cancelled or timed out; inspect package manager state before retrying"
                    .into(),
            );
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status.success()),
            Ok(None) => thread::sleep(Duration::from_millis(40)),
            Err(e) => {
                // Only the process group created for this child is signalled.
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGKILL);
                }
                let _ = child.kill();
                let _ = child.wait();
                break Err(e.to_string());
            }
        }
    };
    // Readers continue draining after the output cap to prevent pipe deadlocks.
    if result.is_err() {
        return result.map(|_| Output {
            success: false,
            text: String::new(),
        });
    }
    let mut out = Vec::new();
    let mut err = Vec::new();
    for _ in 0..2 {
        let remaining = timeout.saturating_sub(start.elapsed());
        match rx.recv_timeout(remaining) {
            Ok((true, bytes)) => out = bytes,
            Ok((false, bytes)) => err = bytes,
            Err(_) => {
                // A descendant can inherit stdout even after its parent exits.
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGKILL);
                }
                return Err("Timed out draining process output".into());
            }
        }
    }
    let success = result?;
    Ok(Output {
        success,
        text: format!(
            "{}{}",
            String::from_utf8_lossy(&out),
            String::from_utf8_lossy(&err)
        ),
    })
}
pub fn query(spec: &Spec) -> Result<Output, String> {
    run(spec, Duration::from_secs(12), &AtomicBool::new(false))
}
pub fn find(name: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    if !crate::catalog::valid_id(name) {
        return None;
    }
    std::env::var_os("PATH").and_then(|p| {
        std::env::split_paths(&p)
            .filter(|p| p.is_absolute())
            .map(|p| p.join(name))
            .find(|p| {
                p.is_file()
                    && p.metadata()
                        .is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
            })
    })
}
pub fn tool_executable(tool: &crate::catalog::Tool) -> Option<PathBuf> {
    crate::edition::bundled_executable(tool)
        .or_else(|| find(&tool.executable))
        .or_else(|| {
            let paths = crate::config::Paths::discover().ok()?;
            let path = paths
                .data
                .join("tools")
                .join(&tool.id)
                .join("usr/bin")
                .join(&tool.executable);
            use std::os::unix::fs::PermissionsExt;
            (path.is_file()
                && path
                    .metadata()
                    .is_ok_and(|m| m.permissions().mode() & 0o111 != 0))
            .then_some(path)
        })
}
pub fn flatpak_installed(tool: &crate::catalog::Tool) -> Option<String> {
    let id = tool.flatpak.as_ref()?;
    let flatpak = find("flatpak")?;
    let out = query(&Spec::new(
        flatpak,
        &["info", "--user", "--show-version", id],
    ))
    .ok()?;
    out.success.then(|| out.text.trim().to_string())
}
pub fn launch(tool: &crate::catalog::Tool, file: Option<&Path>) -> Result<Spec, String> {
    if tool_executable(tool).is_none() && tool.gui && flatpak_installed(tool).is_some() {
        let mut args = vec![
            "run".into(),
            "--user".into(),
            tool.flatpak.clone().ok_or("No Flatpak ID")?,
        ];
        if let Some(file) = file {
            args.push(
                file.canonicalize()
                    .map_err(|e| e.to_string())?
                    .to_string_lossy()
                    .into(),
            );
        }
        let spec = Spec {
            program: find("flatpak").ok_or("Flatpak missing")?,
            args,
        };
        let mut child = Command::new(&spec.program)
            .args(&spec.args)
            .spawn()
            .map_err(|e| e.to_string())?;
        thread::spawn(move || {
            let _ = child.wait();
        });
        return Ok(spec);
    }
    let executable = tool_executable(tool).ok_or("Executable no longer available")?;
    let mut args = Vec::new();
    if let Some(path) = file {
        let canonical = path.canonicalize().map_err(|e| e.to_string())?;
        args.extend(tool.file_args.clone());
        args.push(canonical.to_string_lossy().into());
    }
    let spec = if tool.gui {
        Spec {
            program: executable,
            args,
        }
    } else {
        let (terminal, prefix) = [
            ("gnome-terminal", vec!["--"]),
            ("kgx", vec!["--"]),
            ("xfce4-terminal", vec!["--disable-server", "-x"]),
            ("xterm", vec!["-hold", "-e"]),
        ]
        .into_iter()
        .find_map(|(t, a)| find(t).map(|p| (p, a)))
        .ok_or("Install a supported terminal: gnome-terminal, kgx, xfce4-terminal or xterm")?;
        let mut terminal_args: Vec<String> = prefix.iter().map(|s| s.to_string()).collect();
        if let Some(appdir) = std::env::var_os("RETOOLKIT_APPDIR") {
            terminal_args.push("/usr/bin/env".into());
            terminal_args.push(format!(
                "LD_LIBRARY_PATH={}",
                std::env::var("LD_LIBRARY_PATH").unwrap_or_default()
            ));
            terminal_args.push(format!("RETOOLKIT_APPDIR={}", appdir.to_string_lossy()));
        }
        terminal_args.push(
            std::env::current_exe()
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .into(),
        );
        terminal_args.push("--run-cli".into());
        terminal_args.push(tool.id.clone());
        if let Some(path) = file {
            terminal_args.push(
                path.canonicalize()
                    .map_err(|e| e.to_string())?
                    .to_string_lossy()
                    .into(),
            );
        }
        Spec {
            program: terminal,
            args: terminal_args,
        }
    };
    external_command(&spec.program)
        .args(&spec.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())
        .map(|mut child| {
            thread::spawn(move || {
                let _ = child.wait();
            });
        })?;
    Ok(spec)
}
pub fn save_log(path: &Path, text: &str) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| e.to_string())?;
    writeln!(f, "{text}").map_err(|e| e.to_string())
}
pub type Cancellation = Arc<AtomicBool>;
