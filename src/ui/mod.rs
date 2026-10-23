use adw::prelude::*;
use gtk::{gio, glib};
use retoolkit_linux::{
    catalog::{self, Tool},
    config::{self, Paths, Settings},
    detection::{self, Detected},
    installers::{self, Action, Plan},
    process::{self, Spec},
};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    path::PathBuf,
    rc::Rc,
    sync::{atomic::AtomicBool, mpsc, Arc},
    time::Duration,
};

struct Model {
    tools: Vec<Tool>,
    detected: Vec<Detected>,
    updates: BTreeMap<String, String>,
    paths: Paths,
    settings: Settings,
    page: String,
    query: String,
    busy: bool,
    cancellation: Option<Arc<AtomicBool>>,
    log: String,
    analysis_log: String,
    selected_file: Option<PathBuf>,
}
#[derive(Clone)]
struct View {
    window: adw::ApplicationWindow,
    content: gtk::Box,
    status: gtk::Label,
    spinner: gtk::Spinner,
    model: Rc<RefCell<Model>>,
    tx: mpsc::Sender<Event>,
}
enum Event {
    Scan(Vec<Detected>, BTreeMap<String, String>),
    Plan(Result<Box<Plan>, String>),
    Operation(String, bool),
    Progress(String),
    Analysis(Result<String, String>),
}
fn label(text: &str, class: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.set_xalign(0.0);
    l.set_wrap(true);
    if !class.is_empty() {
        l.add_css_class(class);
    }
    l
}
fn column(spacing: i32) -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Vertical, spacing)
}
fn clear(b: &gtk::Box) {
    while let Some(c) = b.first_child() {
        b.remove(&c);
    }
}
fn text_area(text: &str) -> gtk::ScrolledWindow {
    let t = gtk::TextView::new();
    t.set_editable(false);
    t.set_cursor_visible(false);
    t.set_wrap_mode(gtk::WrapMode::WordChar);
    t.buffer().set_text(text);
    t.set_top_margin(16);
    t.set_left_margin(16);
    t.set_right_margin(16);
    gtk::ScrolledWindow::builder()
        .child(&t)
        .vexpand(true)
        .min_content_height(200)
        .build()
}
fn message(v: &View, title: &str, text: &str) {
    let d = adw::MessageDialog::new(Some(&v.window), Some(title), Some(text));
    d.add_response("close", "Fechar");
    d.set_default_response(Some("close"));
    d.present();
}
fn detail(v: &View, t: &Tool) {
    let m = v.model.borrow();
    let d = m.detected.iter().find(|d| d.id == t.id);
    let text=format!("{}\n\nCategoria: {}\nLicença: {}\nArquiteturas: {}\nSite: {}\nRepositório: {}\nExecutável: {}\nCaminho: {}\nVersão: {}\nAPT: {:?}\nDNF: {:?}\nFlatpak: {:?}\nDependências: {:?}\n\nA disponibilidade é verificada no gerenciador antes de cada operação. Instalações externas não são removidas pelo aplicativo. Plugins e bibliotecas podem não oferecer um executável independente.",t.description,t.category,t.license,t.architectures.join(", "),t.website,t.repository,t.executable,d.and_then(|d|d.path.as_deref()).unwrap_or("Não detectado"),d.and_then(|d|d.version.as_deref()).unwrap_or("Não detectada"),t.apt,t.dnf,t.flatpak,t.dependencies);
    message(v, &t.name, &text);
}
fn begin(v: &View, what: &str) -> bool {
    if v.model.borrow().busy {
        message(
            v,
            "Operação em andamento",
            "Aguarde a operação atual terminar.",
        );
        return false;
    }
    v.model.borrow_mut().busy = true;
    v.status.set_text(what);
    v.spinner.start();
    true
}
fn finish(v: &View, text: &str) {
    v.model.borrow_mut().busy = false;
    v.model.borrow_mut().cancellation = None;
    v.spinner.stop();
    v.status.set_text(text);
}
fn scan(v: &View) {
    if !begin(v, "Verificando ferramentas e versões…") {
        return;
    }
    let tx = v.tx.clone();
    let tools = v.model.borrow().tools.clone();
    std::thread::spawn(move || {
        let sys = detection::system();
        let mut ds = detection::all(&tools);
        let mut updates = BTreeMap::new();
        for (t, d) in tools.iter().zip(ds.iter_mut()) {
            let native = installers::native_version(t, &sys);
            d.package_installed = native.is_some();
            d.available = installers::available(t, &sys);
            if d.version.is_none() {
                d.version = native;
            }
            if let Some(u) = installers::available_update(t, &sys) {
                updates.insert(t.id.clone(), u);
            }
        }
        let _ = tx.send(Event::Scan(ds, updates));
    });
}
fn request_plan(v: &View, t: &Tool, a: Action, flatpak: bool) {
    if !begin(v, "Consultando origem e dependências…") {
        return;
    }
    let tx = v.tx.clone();
    let t = t.clone();
    std::thread::spawn(move || {
        let result = if flatpak {
            installers::flatpak_plan(&t, a)
        } else {
            installers::plan(&t, &detection::system(), a)
        };
        let _ = tx.send(Event::Plan(result.map(Box::new)));
    });
}
fn confirm(v: &View, p: Plan) {
    let body=format!("Ferramenta: {}\nPacote: {}\nOrigem: {}\n\nComando com argumentos estruturados:\n{}\n\n{}\n\nPacotes compartilhados podem afetar outras ferramentas. A operação nativa não será interrompida pela interface; acompanhe o log até concluir.",p.tool_id,p.package,p.source,p.command.display(),p.preview.chars().take(9000).collect::<String>());
    let d = adw::MessageDialog::new(Some(&v.window), Some("Confirmar operação"), Some(&body));
    d.add_responses(&[("cancel", "Cancelar"), ("confirm", "Executar")]);
    d.set_response_appearance("confirm", adw::ResponseAppearance::Destructive);
    d.set_default_response(Some("cancel"));
    d.set_close_response("cancel");
    let v = v.clone();
    d.connect_response(None, move |_, r| {
        if r != "confirm" {
            return;
        }
        if !begin(&v, "Executando — acompanhe em Instalações…") {
            return;
        }
        let tx = v.tx.clone();
        let p = p.clone();
        let paths = v.model.borrow().paths.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut m = v.model.borrow_mut();
            m.log = format!("{}\n", p.command.display());
            if p.cancellable {
                m.cancellation = Some(cancel.clone());
            }
        }
        let progress_tx = tx.clone();
        let observer: process::Observer = Arc::new(move |text| {
            let _ = progress_tx.send(Event::Progress(text.to_string()));
        });
        std::thread::spawn(move || {
            let result = installers::execute_observed(&p, &cancel, Some(observer));
            let (success, log) = match result {
                Ok(s) => (true, s),
                Err(e) => (false, e),
            };
            let full = format!("{}\n{}", p.command.display(), log);
            let _ = process::save_log(&paths.data.join("operations.log"), &full);
            let _ = tx.send(Event::Operation(full, success));
        });
    });
    d.present();
}
fn launch(v: &View, t: &Tool, file: Option<PathBuf>) {
    if !t.gui && file.is_none() {
        let hint = match t.id.as_str() {
            "strace" | "ltrace" => "Informe um programa (ex.: /usr/bin/ls -l) ou um PID (ex.: -p 1234). O programa informado será executado; use apenas alvos de confiança.",
            "yara" => "Informe o arquivo de regras e o alvo, por exemplo: regras.yar amostra.bin. Use aspas em caminhos com espaços.",
            _ => "Informe as opções e os arquivos da ferramenta. Use aspas em caminhos com espaços. Os argumentos são passados diretamente, sem executar um shell.",
        };
        let dialog = adw::MessageDialog::new(
            Some(&v.window),
            Some(&format!("Executar {}", t.name)),
            Some(hint),
        );
        let entry = gtk::Entry::new();
        entry.set_placeholder_text(Some("Argumentos da ferramenta"));
        entry.set_activates_default(true);
        dialog.set_extra_child(Some(&entry));
        dialog.add_response("cancel", "Cancelar");
        dialog.add_response("run", "Executar");
        dialog.set_response_appearance("run", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("run"));
        if process::cli_needs_arguments(t) {
            dialog.set_response_enabled("run", false);
            let weak = dialog.downgrade();
            entry.connect_changed(move |e| {
                if let Some(d) = weak.upgrade() {
                    d.set_response_enabled("run", !e.text().trim().is_empty());
                }
            });
        }
        let vv = v.clone();
        let tool = t.clone();
        dialog.connect_response(None, move |_, response| {
            if response != "run" {
                return;
            }
            let text = entry.text();
            let args = if text.trim().is_empty() {
                Ok(Vec::new())
            } else {
                glib::shell_parse_argv(text.as_str()).map(|args| {
                    args.iter()
                        .map(|a| a.to_string_lossy().into_owned())
                        .collect()
                })
            };
            match args {
                Ok(args) => launch_arguments(&vv, &tool, None, &args),
                Err(error) => message(&vv, "Argumentos inválidos", &error.to_string()),
            }
        });
        dialog.present();
        return;
    }
    launch_arguments(v, t, file, &[]);
}
fn launch_arguments(v: &View, t: &Tool, file: Option<PathBuf>, args: &[String]) {
    match process::launch_with_args(t, file.as_deref(), args) {
        Ok(s) => {
            if let Err(e) = config::record(
                &v.model.borrow().paths,
                &format!("Processo iniciado: {} — {}", t.name, s.display()),
            ) {
                message(v, "Histórico", &e);
            }
            v.status
                .set_text("Processo iniciado; consulte a janela da ferramenta.");
        }
        Err(e) => message(v, "Não foi possível abrir", &e),
    }
}
fn inspect(v: &View, path: PathBuf) {
    if !begin(v, "Análise estática isolada em processo separado…") {
        return;
    }
    v.model.borrow_mut().selected_file = Some(path.clone());
    let tx = v.tx.clone();
    std::thread::spawn(move || {
        let result = std::env::current_exe()
            .map_err(|e| e.to_string())
            .and_then(|exe| {
                process::run(
                    &Spec {
                        program: exe,
                        args: vec!["--inspect".into(), path.to_string_lossy().into()],
                    },
                    Duration::from_secs(20),
                    &AtomicBool::new(false),
                )
            })
            .and_then(|o| if o.success { Ok(o.text) } else { Err(o.text) });
        let _ = tx.send(Event::Analysis(result));
    });
}
fn analyze_page(v: &View) {
    v.content.append(&label("Inspeção estática", "title"));
    v.content.append(&label("Os arquivos permanecem locais. Limite: 64 MiB, 20 segundos, 512 seções e 2.000 strings. Nunca executamos a amostra.","subtitle"));
    let choose = gtk::Button::with_label("Selecionar arquivo…");
    choose.add_css_class("suggested-action");
    let vv = v.clone();
    choose.connect_clicked(move |_| {
        let dialog = gtk::FileDialog::new();
        let vv = vv.clone();
        dialog.open(
            Some(&vv.window.clone()),
            gio::Cancellable::NONE,
            move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        inspect(&vv, path);
                    }
                }
            },
        );
    });
    v.content.append(&choose);
    let m = v.model.borrow();
    if !m.analysis_log.is_empty() {
        v.content.append(&text_area(&m.analysis_log));
    }
    if let Some(path) = &m.selected_file {
        let format = m
            .analysis_log
            .lines()
            .find_map(|l| l.trim().strip_prefix("\"format\": \""))
            .map(|s| s.trim_end_matches([',', '"']))
            .unwrap_or("");
        let tools: Vec<_> = m
            .tools
            .iter()
            .filter(|t| {
                t.formats.iter().any(|s| s == format)
                    && m.detected.iter().any(|d| d.id == t.id && d.path.is_some())
            })
            .cloned()
            .collect();
        let path = path.clone();
        drop(m);
        for t in tools {
            let b = gtk::Button::with_label(&format!("Abrir arquivo com {}", t.name));
            let vv = v.clone();
            let p = path.clone();
            b.connect_clicked(move |_| launch(&vv, &t, Some(p.clone())));
            v.content.append(&b);
        }
    }
}
fn dashboard(v: &View) {
    let m = v.model.borrow();
    v.content
        .append(&label("Seu laboratório, sob controle", "title"));
    v.content.append(&label(
        "Engenharia reversa • análise estática • ferramentas locais",
        "subtitle",
    ));
    let cards = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let installed = m
        .detected
        .iter()
        .filter(|d| d.path.is_some() || d.package_installed)
        .count();
    for (title, value) in [
        ("No catálogo", m.tools.len()),
        ("Detectadas", installed),
        (
            "Disponíveis no APT",
            m.detected
                .iter()
                .filter(|d| d.available && !d.package_installed && d.path.is_none())
                .count(),
        ),
        ("Atualizações APT", m.updates.len()),
    ] {
        let c = column(6);
        c.add_css_class("card");
        c.set_hexpand(true);
        c.append(&label(&value.to_string(), "metric"));
        c.append(&label(title, "subtitle"));
        cards.append(&c);
    }
    v.content.append(&cards);
    let s = detection::system();
    let sys = column(10);
    sys.add_css_class("card");
    sys.append(&label("Sistema detectado", "toolname"));
    sys.append(&label(
        &format!(
            "{}\nKernel {} • {}",
            s.distribution, s.kernel, s.architecture
        ),
        "",
    ));
    for (name, available) in s.dependencies {
        sys.append(&label(
            &format!("{}  {}", if available { "●" } else { "○" }, name),
            if available { "installed" } else { "subtitle" },
        ));
    }
    v.content.append(&sys);
    v.content.append(&label("Execuções recentes", "toolname"));
    let history = config::history(&m.paths);
    v.content.append(&label(
        if history.is_empty() {
            "Nenhuma execução registrada."
        } else {
            &history
        },
        "subtitle",
    ));
    v.content.append(&label("As atualizações APT usam o cache local. Atualize os índices pelo gerenciador do sistema. A ausência de atualização detectada não prova que a ferramenta está na versão mais recente.","subtitle"));
}
fn d_is_user_flatpak(v: &View, t: &Tool) -> bool {
    v.model
        .borrow()
        .detected
        .iter()
        .find(|d| d.id == t.id)
        .and_then(|d| d.path.as_deref())
        .is_some_and(|p| p.starts_with("Flatpak (user):"))
}
fn catalog_page(v: &View) {
    let m = v.model.borrow();
    v.content.append(&label(&m.page, "title"));
    let q = m.query.to_lowercase();
    let page = m.page.clone();
    let tools: Vec<_> = m
        .tools
        .iter()
        .filter(|t| {
            (page == "Todas as ferramentas" || page == t.category)
                && (format!("{} {}", t.name, t.description)
                    .to_lowercase()
                    .contains(&q))
        })
        .cloned()
        .collect();
    drop(m);
    if tools.is_empty() {
        v.content.append(&label(
            "Nenhuma ferramenta corresponde à pesquisa.",
            "subtitle",
        ));
    }
    for t in tools {
        let m = v.model.borrow();
        let d = m.detected.iter().find(|d| d.id == t.id);
        let installed = d.is_some_and(|d| d.path.is_some() || d.package_installed);
        let launchable = d.is_some_and(|d| d.path.is_some());
        let managed = d.is_some_and(|d| d.package_installed);
        let available = d.is_some_and(|d| d.available);
        let version = d
            .and_then(|d| d.version.as_deref())
            .unwrap_or("Versão não detectada")
            .to_string();
        let update = m.updates.contains_key(&t.id);
        drop(m);
        let row = column(10);
        row.add_css_class("card");
        let top = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let title = label(&t.name, "toolname");
        title.set_hexpand(true);
        top.append(&title);
        top.append(&label(
            if installed {
                "● Detectada"
            } else {
                "○ Não detectada"
            },
            if installed { "installed" } else { "subtitle" },
        ));
        row.append(&top);
        row.append(&label(&t.description, "subtitle"));
        row.append(&label(&format!("{} · {}", t.category, version), "badge"));
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        if launchable {
            let b = gtk::Button::with_label(if t.gui { "Abrir" } else { "Executar…" });
            let vv = v.clone();
            let tt = t.clone();
            b.connect_clicked(move |_| launch(&vv, &tt, None));
            actions.append(&b);
        }
        // Only advertise native operations if a mapping exists. The preview probes actual availability.
        let sys = detection::system();
        let mapped = if ["ubuntu", "debian", "linuxmint", "pop"].contains(&sys.id.as_str()) {
            t.apt.is_some()
        } else if sys.id == "fedora" {
            t.dnf.is_some()
        } else {
            false
        };
        if mapped {
            if !installed && available {
                let b = gtk::Button::with_label("Verificar instalação…");
                b.add_css_class("suggested-action");
                let vv = v.clone();
                let tt = t.clone();
                b.connect_clicked(move |_| request_plan(&vv, &tt, Action::Install, false));
                actions.append(&b);
            }
            if update {
                let b = gtk::Button::with_label("Atualizar…");
                let vv = v.clone();
                let tt = t.clone();
                b.connect_clicked(move |_| request_plan(&vv, &tt, Action::Update, false));
                actions.append(&b);
            }
            if managed {
                let b = gtk::Button::with_label("Remover pacote…");
                let vv = v.clone();
                let tt = t.clone();
                b.connect_clicked(move |_| request_plan(&vv, &tt, Action::Remove, false));
                actions.append(&b);
            }
        }
        if t.flatpak.is_some() && process::find("flatpak").is_some() {
            let b = gtk::Button::with_label("Verificar Flatpak…");
            let vv = v.clone();
            let tt = t.clone();
            b.connect_clicked(move |_| request_plan(&vv, &tt, Action::Install, true));
            actions.append(&b);
        }
        if t.flatpak.is_some() && d_is_user_flatpak(v, &t) {
            for (title, action) in [
                ("Atualizar Flatpak…", Action::Update),
                ("Remover Flatpak…", Action::Remove),
            ] {
                let b = gtk::Button::with_label(title);
                let vv = v.clone();
                let tt = t.clone();
                b.connect_clicked(move |_| request_plan(&vv, &tt, action.clone(), true));
                actions.append(&b);
            }
        }
        if !installed && t.user_deb.is_some() && sys.id == "ubuntu" {
            let b = gtk::Button::with_label("Instalar por usuário…");
            let vv = v.clone();
            let tt = t.clone();
            b.connect_clicked(move |_| {
                if !begin(&vv, "Verificando pacote oficial para usuário…") {
                    return;
                }
                let tx = vv.tx.clone();
                let tool = tt.clone();
                std::thread::spawn(move || {
                    let _ = tx.send(Event::Plan(installers::user_deb::plan(&tool).map(Box::new)));
                });
            });
            actions.append(&b);
        }
        let b = gtk::Button::with_label("Detalhes");
        let vv = v.clone();
        let tt = t.clone();
        b.connect_clicked(move |_| detail(&vv, &tt));
        actions.append(&b);
        let b = gtk::Button::with_label("Site oficial ↗");
        let vv = v.clone();
        let site = t.website.clone();
        b.connect_clicked(move |_| {
            if let Err(e) = gio::AppInfo::launch_default_for_uri(&site, gio::AppLaunchContext::NONE)
            {
                message(&vv, "Não foi possível abrir o site", &e.to_string());
            }
        });
        actions.append(&b);
        row.append(&actions);
        v.content.append(&row);
    }
}
fn install_user(v: &View, desktop: bool) {
    if retoolkit_linux::edition::current().full() {
        message(v,"Instalação da edição Full","Use o pacote DEB/RPM Full para instalação no sistema ou mantenha o AppImage Full para uso portátil. Este instalador por usuário copia somente a edição Light e não deve separar a Full de suas ferramentas.");
        return;
    }
    let d=adw::MessageDialog::new(Some(&v.window),Some("Instalar para este usuário?"),Some("Copia o executável e o ícone para $XDG_DATA_HOME/retoolkit-linux/app e cria o lançador no menu. Não inclui bibliotecas GTK: requer GTK ≥ 4.10 e libadwaita ≥ 1.4. Nenhum arquivo existente será sobrescrito. O atalho opcional usa xdg-user-dir DESKTOP. Não exige root."));
    d.add_responses(&[("cancel", "Cancelar"), ("install", "Instalar")]);
    d.set_default_response(Some("cancel"));
    let vv = v.clone();
    d.connect_response(None,move|_,r|{if r!="install"{return;}match user_install(&vv.model.borrow().paths,desktop){Ok(p)=>message(&vv,"Instalação concluída",&format!("Lançador criado: {}\nAbra pelo menu de aplicativos. No GNOME, o atalho da Área de Trabalho pode precisar de ‘Permitir iniciar’ e da extensão de ícones.",p.display())),Err(e)=>message(&vv,"Falha na instalação",&e)}});
    d.present();
}
fn desktop_quote(path: &std::path::Path) -> String {
    format!(
        "\"{}\"",
        path.to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('`', "\\`")
            .replace('$', "\\$")
            .replace('%', "%%")
    )
}
fn user_install(paths: &Paths, desktop: bool) -> Result<PathBuf, String> {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    paths.create()?;
    let app = paths.data.join("app");
    std::fs::create_dir_all(&app).map_err(|e| e.to_string())?;
    let executable = app.join("retoolkit-linux");
    let icon = app.join("retoolkit-linux.svg");
    let data_home = paths.data.parent().ok_or("No data directory")?;
    let applications = data_home.join("applications");
    std::fs::create_dir_all(&applications).map_err(|e| e.to_string())?;
    let launcher = applications.join("io.github.retoolkitlinux.REtoolkit.desktop");
    if executable.exists() || icon.exists() || launcher.exists() {
        return Err(
            "An installation already exists. Remove or update it explicitly; files were preserved."
                .into(),
        );
    }
    let mut src = std::fs::File::open(std::env::current_exe().map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let mut dst = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&executable)
        .map_err(|e| e.to_string())?;
    std::io::copy(&mut src, &mut dst).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| e.to_string())?;
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&icon)
        .map_err(|e| e.to_string())?
        .write_all(include_bytes!("../../assets/icons/retoolkit-linux.svg"))
        .map_err(|e| e.to_string())?;
    let contents=format!("[Desktop Entry]\nType=Application\nName=REtoolkit Linux\nComment=Reverse engineering workstation\nExec={}\nIcon={}\nTerminal=false\nCategories=Development;Debugger;\n",desktop_quote(&executable),icon.display());
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&launcher)
        .map_err(|e| e.to_string())?
        .write_all(contents.as_bytes())
        .map_err(|e| e.to_string())?;
    if desktop {
        let p = process::find("xdg-user-dir")
            .ok_or("xdg-user-dir unavailable; menu launcher was created")?;
        let out = process::query(&Spec::new(p, &["DESKTOP"]))?;
        let folder = PathBuf::from(out.text.trim());
        if !out.success || !folder.is_absolute() || !folder.is_dir() {
            return Err("Desktop directory unavailable; menu launcher was created".into());
        }
        let shortcut = folder.join("REtoolkit Linux.desktop");
        let mut f = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&shortcut)
            .map_err(|e| e.to_string())?;
        f.write_all(contents.as_bytes())
            .map_err(|e| e.to_string())?;
        std::fs::set_permissions(shortcut, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }
    Ok(launcher)
}
fn settings_page(v: &View) {
    v.content.append(&label("Configurações", "title"));
    let m = v.model.borrow();
    v.content.append(&label(
        &format!(
            "Configuração: {}\nDados e logs: {}\nCache: {}",
            m.paths.config.display(),
            m.paths.data.display(),
            m.paths.cache.display()
        ),
        "subtitle",
    ));
    let theme = gtk::CheckButton::with_label("Seguir o tema do sistema");
    theme.set_active(m.settings.follow_system_theme);
    drop(m);
    let vv = v.clone();
    theme.connect_toggled(move |b| {
        let mut m = vv.model.borrow_mut();
        m.settings.follow_system_theme = b.is_active();
        if let Err(e) = m.settings.save(&m.paths) {
            drop(m);
            message(&vv, "Erro ao salvar", &e);
            return;
        }
        if b.is_active() {
            vv.window.remove_css_class("retoolkit-dark");
        } else {
            vv.window.add_css_class("retoolkit-dark");
        }
        adw::StyleManager::default().set_color_scheme(if b.is_active() {
            adw::ColorScheme::Default
        } else {
            adw::ColorScheme::ForceDark
        });
    });
    v.content.append(&theme);
    v.content
        .append(&label(retoolkit_linux::updates::STATUS, "subtitle"));
    let shortcut = gtk::CheckButton::with_label("Criar também atalho na Área de Trabalho");
    v.content.append(&shortcut);
    let b = gtk::Button::with_label("Instalar aplicativo para meu usuário…");
    let vv = v.clone();
    b.connect_clicked(move |_| install_user(&vv, shortcut.is_active()));
    v.content.append(&b);
    let edition = retoolkit_linux::edition::current();
    v.content.append(&label(&format!("REtoolkit Linux {} {}\nFerramentas incluídas: {}\nFerramentas do gerenciador: {}\nNão é uma sandbox. Use uma VM isolada para análise dinâmica.",edition.label(),edition.version,if edition.bundled_tools.is_empty(){"nenhuma".into()}else{edition.bundled_tools.join(", ")},if edition.distribution_tools.is_empty(){"sob demanda".into()}else{edition.distribution_tools.join(", ")}),"subtitle"));
}
fn render(v: &View) {
    clear(&v.content);
    let page = v.model.borrow().page.clone();
    match page.as_str() {
        "Dashboard" => dashboard(v),
        "Analisador de Arquivos" => analyze_page(v),
        "Instalações" => {
            v.content.append(&label("Operações e diagnóstico", "title"));
            let log = v.model.borrow().log.clone();
            v.content.append(&label("As operações nativas não podem ser canceladas com segurança durante a transação. Autorize pelo polkit e aguarde a conclusão.","subtitle"));
            let paths = v.model.borrow().paths.clone();
            let stored = if v.model.borrow().busy {
                String::new()
            } else {
                std::fs::read_to_string(paths.data.join("operations.log")).unwrap_or_default()
            };
            if let Some(cancel) = v.model.borrow().cancellation.clone() {
                let b = gtk::Button::with_label("Cancelar operação por usuário");
                b.connect_clicked(move |_| {
                    cancel.store(true, std::sync::atomic::Ordering::Relaxed)
                });
                v.content.append(&b);
            }
            v.content
                .append(&text_area(if stored.is_empty() { &log } else { &stored }));
        }
        "Configurações" => settings_page(v),
        _ => catalog_page(v),
    }
}
pub fn run(smoke: bool) {
    let app = adw::Application::builder()
        .application_id("io.github.retoolkitlinux.REtoolkit")
        .build();
    app.connect_activate(move |app| build(app, smoke));
    app.run_with_args(&["retoolkit-linux"]);
}
fn build(app: &adw::Application, smoke: bool) {
    let paths = match Paths::discover().and_then(|p| {
        p.create()?;
        Ok(p)
    }) {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("{e}");
            return;
        }
    };
    let settings = match Settings::load(&paths) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("{e}");
            Settings::default()
        }
    };
    adw::StyleManager::default().set_color_scheme(if settings.follow_system_theme {
        adw::ColorScheme::Default
    } else {
        adw::ColorScheme::ForceDark
    });
    let provider = gtk::CssProvider::new();
    provider.load_from_data(include_str!("../../assets/styles/app.css"));
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
    let tools = match catalog::load() {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("{e}");
            return;
        }
    };
    let model = Rc::new(RefCell::new(Model {
        tools,
        detected: vec![],
        updates: BTreeMap::new(),
        paths,
        settings,
        page: "Dashboard".into(),
        query: String::new(),
        busy: false,
        cancellation: None,
        log: String::new(),
        analysis_log: String::new(),
        selected_file: None,
    }));
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title(format!(
            "REtoolkit Linux {}",
            retoolkit_linux::edition::current().label()
        ))
        .default_width(1250)
        .default_height(850)
        .build();
    if !model.borrow().settings.follow_system_theme {
        window.add_css_class("retoolkit-dark");
    }
    let root = column(0);
    let header = adw::HeaderBar::new();
    let search = gtk::SearchEntry::builder()
        .placeholder_text("Pesquisar ferramentas…")
        .width_request(360)
        .build();
    header.set_title_widget(Some(&search));
    let refresh = gtk::Button::from_icon_name("view-refresh-symbolic");
    refresh.set_tooltip_text(Some("Verificar ferramentas novamente"));
    header.pack_end(&refresh);
    root.append(&header);
    let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    body.set_vexpand(true);
    let sidebar = column(2);
    sidebar.add_css_class("sidebar");
    sidebar.set_width_request(220);
    sidebar.append(&label("REtoolkit", "brand"));
    sidebar.append(&label("LINUX  /  WORKSTATION", "tagline"));
    body.append(&sidebar);
    let content = column(16);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content.set_margin_start(24);
    content.set_margin_end(24);
    let scroll = gtk::ScrolledWindow::builder()
        .child(&content)
        .hexpand(true)
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    body.append(&scroll);
    root.append(&body);
    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    footer.set_margin_start(16);
    footer.set_margin_bottom(8);
    let spinner = gtk::Spinner::new();
    footer.append(&spinner);
    let status = label("Pronto", "subtitle");
    footer.append(&status);
    root.append(&footer);
    window.set_content(Some(&root));
    let (tx, rx) = mpsc::channel();
    let v = View {
        window: window.clone(),
        content,
        status,
        spinner,
        model,
        tx,
    };
    let drop_target = gtk::DropTarget::new(gio::File::static_type(), gtk::gdk::DragAction::COPY);
    let vv = v.clone();
    drop_target.connect_drop(move |_, value, _, _| {
        if vv.model.borrow().page != "Analisador de Arquivos" {
            return false;
        }
        if let Ok(file) = value.get::<gio::File>() {
            if let Some(path) = file.path() {
                inspect(&vv, path);
                return true;
            }
        }
        false
    });
    v.content.add_controller(drop_target);
    let mut group: Option<gtk::ToggleButton> = None;
    for page in [
        "Dashboard",
        "Todas as ferramentas",
        "Engenharia Reversa",
        "Análise de Malware",
        "Debuggers",
        "Análise de Firmware",
        "Android",
        "Utilitários",
        "Analisador de Arquivos",
        "Instalações",
        "Configurações",
    ] {
        let b = gtk::ToggleButton::with_label(page);
        if let Some(g) = &group {
            b.set_group(Some(g));
        } else {
            group = Some(b.clone());
            b.set_active(true);
        }
        let vv = v.clone();
        b.connect_toggled(move |b| {
            if b.is_active() {
                vv.model.borrow_mut().page = page.into();
                render(&vv);
            }
        });
        sidebar.append(&b);
    }
    let settings_button = gtk::Button::from_icon_name("emblem-system-symbolic");
    settings_button.set_tooltip_text(Some("Configurações"));
    header.pack_end(&settings_button);
    let vv = v.clone();
    let nav = sidebar.clone();
    settings_button.connect_clicked(move |_| {
        let mut child = nav.first_child();
        while let Some(c) = child {
            if let Ok(b) = c.clone().downcast::<gtk::ToggleButton>() {
                if b.label().as_deref() == Some("Configurações") {
                    b.set_active(true);
                    return;
                }
            }
            child = c.next_sibling();
        }
        vv.model.borrow_mut().page = "Configurações".into();
        render(&vv);
    });
    let controller = gtk::EventControllerKey::new();
    let focus_search = search.clone();
    let close_window = window.clone();
    controller.connect_key_pressed(move |_, key, _, state| {
        if state.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
            if key == gtk::gdk::Key::f {
                focus_search.grab_focus();
                return glib::Propagation::Stop;
            }
            if key == gtk::gdk::Key::q {
                close_window.close();
                return glib::Propagation::Stop;
            }
        }
        glib::Propagation::Proceed
    });
    window.add_controller(controller);
    let vv = v.clone();
    refresh.connect_clicked(move |_| scan(&vv));
    let vv = v.clone();
    let nav = sidebar.clone();
    search.connect_search_changed(move |s| {
        let mut m = vv.model.borrow_mut();
        m.query = s.text().to_string();
        m.page = "Todas as ferramentas".into();
        drop(m);
        let mut child = nav.first_child();
        while let Some(c) = child {
            if let Ok(b) = c.clone().downcast::<gtk::ToggleButton>() {
                if b.label().as_deref() == Some("Todas as ferramentas") {
                    b.set_active(true);
                    break;
                }
            }
            child = c.next_sibling();
        }

        render(&vv);
    });
    let vv = v.clone();
    window.connect_close_request(move |_| {
        if vv.model.borrow().busy {
            message(
                &vv,
                "Operação em andamento",
                "Aguarde a conclusão antes de fechar o aplicativo.",
            );
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    let vv = v.clone();
    glib::timeout_add_local(Duration::from_millis(100), move || {
        while let Ok(event) = rx.try_recv() {
            match event {
                Event::Scan(ds, updates) => {
                    let mut m = vv.model.borrow_mut();
                    m.detected = ds;
                    m.updates = updates;
                    drop(m);
                    finish(&vv, "Verificação concluída");
                    render(&vv);
                }
                Event::Plan(result) => {
                    finish(&vv, "Plano pronto");
                    match result {
                        Ok(p) => confirm(&vv, *p),
                        Err(e) => message(&vv, "Operação indisponível", &e),
                    }
                }
                Event::Operation(log, success) => {
                    vv.model.borrow_mut().log = log;
                    finish(
                        &vv,
                        if success {
                            "Comando concluído; verificando resultado…"
                        } else {
                            "Operação falhou; consulte Instalações"
                        },
                    );
                    if success {
                        scan(&vv);
                    } else {
                        render(&vv);
                    }
                }
                Event::Progress(text) => {
                    let mut m = vv.model.borrow_mut();
                    m.log.push_str(&text);
                    if m.log.len() > 128 * 1024 {
                        let excess = m.log.len() - 128 * 1024;
                        let mut at = excess;
                        while !m.log.is_char_boundary(at) {
                            at += 1;
                        }
                        m.log.drain(..at);
                    }
                    let show = m.page == "Instalações";
                    drop(m);
                    if show {
                        render(&vv);
                    }
                }
                Event::Analysis(result) => {
                    let mut m = vv.model.borrow_mut();
                    m.analysis_log = match result {
                        Ok(s) => s,
                        Err(e) => format!("Análise falhou: {e}"),
                    };
                    m.page = "Analisador de Arquivos".into();
                    drop(m);
                    finish(&vv, "Análise concluída");
                    render(&vv);
                }
            }
        }
        if vv.window.is_visible() {
            glib::ControlFlow::Continue
        } else {
            glib::ControlFlow::Break
        }
    });
    render(&v);
    window.present();
    scan(&v);
    if smoke {
        smoke_test(&v, &search);
    }
}

fn smoke_test(v: &View, search: &gtk::SearchEntry) {
    let vv = v.clone();
    let search = search.clone();
    let mut stage = 0;
    let started = std::time::Instant::now();
    glib::timeout_add_local(Duration::from_millis(500), move || {
        if started.elapsed() > Duration::from_secs(60) {
            eprintln!("GUI smoke timeout");
            std::process::exit(1);
        }
        if vv.model.borrow().busy {
            return glib::ControlFlow::Continue;
        }
        match stage {
            0 => {
                assert!(!vv.model.borrow().detected.is_empty());
                println!("PASS GUI startup and detection");
                let tool = vv
                    .model
                    .borrow()
                    .tools
                    .iter()
                    .find(|t| t.id == "strace")
                    .unwrap()
                    .clone();
                launch(&vv, &tool, None);
                let dialog = gtk::Window::list_toplevels()
                    .into_iter()
                    .filter_map(|w| w.downcast::<adw::MessageDialog>().ok())
                    .find(|d| d.heading().as_deref() == Some("Executar strace"))
                    .expect("strace arguments dialog");
                assert!(!dialog.is_response_enabled("run"));
                let entry = dialog
                    .extra_child()
                    .unwrap()
                    .downcast::<gtk::Entry>()
                    .unwrap();
                entry.set_text("/usr/bin/true");
                assert!(dialog.is_response_enabled("run"));
                dialog.response("cancel");
                println!("PASS GUI strace asks for arguments and blocks an empty command");
                search.set_text("yara");
            }
            1 => {
                assert_eq!(vv.model.borrow().query, "yara");
                let mut count = 0;
                let mut child = vv.content.first_child();
                while let Some(c) = child {
                    count += 1;
                    child = c.next_sibling();
                }
                assert_eq!(count, 2);
                println!("PASS GUI catalog search");
                inspect(&vv, PathBuf::from("/usr/bin/ls"));
            }
            2 => {
                assert!(vv
                    .model
                    .borrow()
                    .analysis_log
                    .contains("\"format\": \"ELF\""));
                println!("PASS GUI isolated ELF analysis");
                if process::find("gnome-terminal").is_some() {
                    let tool = vv
                        .model
                        .borrow()
                        .tools
                        .iter()
                        .find(|t| t.id == "file")
                        .cloned()
                        .expect("file catalog entry");
                    launch(&vv, &tool, Some(PathBuf::from("/usr/bin/ls")));
                }

                vv.model.borrow_mut().page = "Configurações".into();
                render(&vv);
            }
            3 => {
                vv.model.borrow_mut().page = "Instalações".into();
                render(&vv);
                if process::find("gnome-terminal").is_some() {
                    let history = config::history(&vv.model.borrow().paths);
                    if !history.contains("CLI concluída: file") {
                        return glib::ControlFlow::Continue;
                    }
                    println!("PASS GUI terminal launch and actual file command completion");
                }
                println!("PASS GUI settings and operations pages");
                vv.window.close();
                return glib::ControlFlow::Break;
            }
            _ => unreachable!(),
        }
        stage += 1;
        glib::ControlFlow::Continue
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn user_launcher_uses_xdg_and_preserves_existing_files() {
        let base = std::env::temp_dir().join(format!("retoolkit-launcher-{}", std::process::id()));
        let paths = Paths {
            config: base.join("config/retoolkit-linux"),
            data: base.join("data/retoolkit-linux"),
            cache: base.join("cache/retoolkit-linux"),
        };
        let launcher = user_install(&paths, false).unwrap();
        let text = std::fs::read_to_string(&launcher).unwrap();
        assert!(text.contains("Terminal=false"));
        assert!(text.contains(&format!("Exec=\"{}", paths.data.display())));
        let output = std::process::Command::new("desktop-file-validate")
            .arg(&launcher)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(user_install(&paths, false).is_err());
        std::fs::remove_dir_all(base).unwrap();
    }
}
