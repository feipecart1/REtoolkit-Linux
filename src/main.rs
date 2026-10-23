mod ui;
use retoolkit_linux::{analysis, catalog, detection};
fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|s| s == "--inspect") {
        // Limits apply only to the isolated parser process, never to the GTK process.
        unsafe {
            let memory = libc::rlimit {
                rlim_cur: 384 * 1024 * 1024,
                rlim_max: 384 * 1024 * 1024,
            };
            let cpu = libc::rlimit {
                rlim_cur: 10,
                rlim_max: 10,
            };
            if libc::setrlimit(libc::RLIMIT_AS, &memory) != 0
                || libc::setrlimit(libc::RLIMIT_CPU, &cpu) != 0
            {
                eprintln!("Unable to apply parser resource limits");
                std::process::exit(1);
            }
        }
        let result = args
            .get(2)
            .ok_or("Missing file path".to_string())
            .and_then(|p| analysis::inspect(std::path::Path::new(p)));
        match result {
            Ok(r) => println!(
                "{}",
                serde_json::to_string_pretty(&r).expect("serializable report")
            ),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }
    if args
        .get(1)
        .is_some_and(|s| s == "--install-user-tool" || s == "--plan-user-tool")
    {
        let result = (|| -> Result<(), String> {
            let tools = catalog::load()?;
            let id = args
                .get(2)
                .and_then(|s| s.to_str())
                .ok_or("Missing tool ID")?;
            let tool = tools.iter().find(|t| t.id == id).ok_or("Unknown tool")?;
            let plan = retoolkit_linux::installers::user_deb::plan(tool)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&plan).map_err(|e| e.to_string())?
            );
            if args[1] == "--install-user-tool" {
                if !args.get(3).is_some_and(|s| s == "--yes") {
                    return Err("Review the plan, then explicitly pass --yes to install".into());
                }
                let output = retoolkit_linux::installers::execute(
                    &plan,
                    &std::sync::atomic::AtomicBool::new(false),
                )?;
                println!("{output}");
            }
            Ok(())
        })();
        if let Err(e) = result {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return;
    }
    if args.get(1).is_some_and(|s| s == "--edition") {
        println!(
            "{}",
            serde_json::to_string_pretty(&retoolkit_linux::edition::current())
                .expect("edition JSON")
        );
        return;
    }
    if args.get(1).is_some_and(|s| s == "--diagnose") {
        match catalog::load() {
            Ok(tools) => println!(
                "{}",
                serde_json::json!({"edition":retoolkit_linux::edition::current(),"system":detection::system(),"tools":detection::all(&tools)})
            ),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }
    if args.get(1).is_some_and(|s| s == "--run-cli") {
        let result = (|| -> Result<(), String> {
            let id = args
                .get(2)
                .and_then(|s| s.to_str())
                .ok_or("Missing tool ID")?;
            let tools = catalog::load()?;
            let tool = tools
                .iter()
                .find(|t| t.id == id && !t.gui)
                .ok_or("Unknown CLI tool")?;
            let executable =
                retoolkit_linux::process::tool_executable(tool).ok_or("Tool unavailable")?;
            let mut command = retoolkit_linux::process::external_command(executable);
            if let Some(file) = args.get(3) {
                if tool.formats.is_empty() {
                    return Err("File opening is not supported for this tool".into());
                }
                let file = std::path::Path::new(file)
                    .canonicalize()
                    .map_err(|e| e.to_string())?;
                command.args(&tool.file_args).arg(file);
            }
            let status = command.status().map_err(|e| e.to_string())?;
            println!("\nProcess exited: {status}");
            if let Ok(paths) = retoolkit_linux::config::Paths::discover() {
                let _ = retoolkit_linux::config::record(
                    &paths,
                    &format!("CLI concluída: {} — {status}", tool.name),
                );
            }
            Ok(())
        })();
        if let Err(error) = result {
            eprintln!("{error}");
        }
        println!("\nPressione Enter para fechar o terminal.");
        let mut line = String::new();
        let _ = std::io::stdin().read_line(&mut line);
        return;
    }
    if args.get(1).is_some_and(|s| s == "--version") {
        println!(
            "REtoolkit Linux {} {}",
            retoolkit_linux::edition::current().label(),
            env!("CARGO_PKG_VERSION")
        );
        return;
    }
    ui::run(args.get(1).is_some_and(|s| s == "--gui-smoke-test"));
}
