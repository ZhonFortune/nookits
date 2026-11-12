#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{collections::{HashMap, HashSet}, fs::{self, OpenOptions}, io::Write, path::{Path, PathBuf}, process::{Child, Command, Stdio}, sync::{Arc, Mutex}, time::Duration};
use notify::Watcher;
use serde::{Deserialize, Serialize};
use tauri::{menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu}, tray::TrayIconBuilder, AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use tauri_plugin_opener::OpenerExt;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Preferences { language: String }

struct State {
    data: PathBuf,
    preferences: Mutex<Preferences>,
    running: Mutex<HashSet<PathBuf>>,
    children: Mutex<Vec<Arc<Mutex<Child>>>>,
    scripts: Mutex<HashMap<String, PathBuf>>,
}

fn strings(language: &str) -> HashMap<String, String> {
    serde_json::from_str(if language == "zh-CN" {
        include_str!("../../locales/zh-CN.json")
    } else { include_str!("../../locales/en.json") }).expect("valid bundled translations")
}

fn message(app: &AppHandle, key: &str) {
    let state = app.state::<State>();
    let language = state.preferences.lock().unwrap().language.clone();
    let text = strings(&language);
    app.dialog().message(&text[key]).title("Nookits").show(|_| {});
}

fn show_about(app: &AppHandle) {
    let language = app.state::<State>().preferences.lock().unwrap().language.clone();
    let text = strings(&language);
    let repository = env!("NOOKIT_GITHUB_URL");
    let content = format!("Nookits {}\n\n{}", app.package_info().version, text["aboutDescription"]);
    let handle = app.clone();
    app.dialog().message(content).title(&text["about"])
        .buttons(MessageDialogButtons::OkCancelCustom(
            text["repository"].clone(), text["confirm"].clone(),
        ))
        .show(move |open| {
            if !open {
                return;
            }
            if repository.is_empty() {
                message(&handle, "repositoryUnavailable");
                return;
            }
            if let Err(error) = handle.opener().open_url(repository, None::<&str>) {
                eprintln!("{error}");
                message(&handle, "error");
            }
        });
}

fn save_preferences(state: &State, preferences: &Preferences) -> Result<()> {
    let target = state.data.join("settings.json");
    fs::write(&target, serde_json::to_vec_pretty(preferences)?)?;
    Ok(())
}

fn initialize_scripts(scripts: &Path, examples: &Path) -> std::io::Result<()> {
    let first_start = !scripts.try_exists()?;
    fs::create_dir_all(scripts)?;
    if !first_start {
        return Ok(());
    }

    for entry in fs::read_dir(examples)? {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type()?.is_file()
            || !path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("js"))
        {
            continue;
        }

        let content = fs::read(&path)?;
        let target = scripts.join(entry.file_name());
        match OpenOptions::new().write(true).create_new(true).open(target) {
            Ok(mut file) => file.write_all(&content)?,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn refresh(app: &AppHandle) -> Result<()> {
    let state = app.state::<State>();
    let language = state.preferences.lock().unwrap().language.clone();
    let text = strings(&language);
    let menu = Menu::new(app)?;
    let mut paths = fs::read_dir(state.data.join("scripts"))?
        .filter_map(|entry| entry.ok()).map(|entry| entry.path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("js")))
        .collect::<Vec<_>>();
    paths.sort_by_key(|path| path.file_name().map(|name| name.to_string_lossy().to_lowercase()));
    let mut scripts = HashMap::new();
    for (index, path) in paths.into_iter().enumerate() {
        let id = format!("script:{index}");
        let name = path.file_stem().unwrap_or_default().to_string_lossy();
        menu.append(&MenuItem::with_id(app, &id, name, true, None::<&str>)?)?;
        scripts.insert(id, path);
    }
    if scripts.is_empty() { menu.append(&MenuItem::new(app, &text["empty"], false, None::<&str>)?)?; }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, "open", &text["openScripts"], true, None::<&str>)?)?;
    let settings = Submenu::new(app, &text["settings"], true)?;
    let languages = Submenu::new(app, &text["language"], true)?;
    for (code, label) in [("en", "English"), ("zh-CN", "简体中文")] {
        languages.append(&CheckMenuItem::with_id(app, format!("language:{code}"), label, true, language == code, None::<&str>)?)?;
    }
    settings.append(&languages)?;
    settings.append(&CheckMenuItem::with_id(app, "autostart", &text["autostart"], true, app.autolaunch().is_enabled()?, None::<&str>)?)?;
    menu.append(&settings)?;
    menu.append(&MenuItem::with_id(app, "about", &text["about"], true, None::<&str>)?)?;
    menu.append(&MenuItem::with_id(app, "quit", &text["quit"], true, None::<&str>)?)?;
    if let Some(tray) = app.tray_by_id("main") { tray.set_menu(Some(menu))?; }
    *state.scripts.lock().unwrap() = scripts;
    Ok(())
}

fn run_script(app: &AppHandle, script: PathBuf) -> Result<()> {
    let state = app.state::<State>();
    if !state.running.lock().unwrap().insert(script.clone()) { message(app, "busy"); return Ok(()); }
    let result = (|| -> Result<_> {
        let executable = std::env::current_exe()?.parent().ok_or("Missing executable directory")?
            .join(format!("nookits-node{}", std::env::consts::EXE_SUFFIX));
        let runner = app.path().resource_dir()?.join("runtime/runtime.mjs");
        let log_path = state.data.join("runtime.log");
        if fs::metadata(&log_path).is_ok_and(|meta| meta.len() > 1_048_576) { fs::write(&log_path, b"")?; }
        let mut log = OpenOptions::new().create(true).append(true).open(log_path)?;
        writeln!(log, "\nRunning {}", script.display())?;
        let mut command = Command::new(executable);
        command.arg(runner).arg(&script).arg(&state.data).current_dir(state.data.join("scripts"))
            .stdin(Stdio::null()).stdout(log.try_clone()?).stderr(log);
        #[cfg(windows)] {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        Ok(command.spawn()?)
    })();
    let child = match result {
        Ok(child) => child,
        Err(error) => { state.running.lock().unwrap().remove(&script); return Err(error); }
    };
    let child = Arc::new(Mutex::new(child));
    state.children.lock().unwrap().push(child.clone());
    let app = app.clone();
    std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(300);
        let success = loop {
            let mut child = child.lock().unwrap();
            match child.try_wait() {
                Ok(Some(status)) => break status.success(),
                Ok(None) if std::time::Instant::now() < deadline => std::thread::sleep(Duration::from_millis(200)),
                _ => { let _ = child.kill(); let _ = child.wait(); break false; }
            }
        };
        app.state::<State>().children.lock().unwrap().retain(|item| !Arc::ptr_eq(item, &child));
        app.state::<State>().running.lock().unwrap().remove(&script);
        if !success { message(&app, "scriptError"); }
    });
    Ok(())
}

fn handle_menu(app: &AppHandle, id: &str) -> Result<()> {
    let state = app.state::<State>();
    match id {
        "about" => show_about(app),
        "quit" => {
            for child in state.children.lock().unwrap().iter() {
                let mut child = child.lock().unwrap();
                let _ = child.kill();
                let _ = child.wait();
            }
            app.exit(0);
        }
        "open" => app.opener().open_path(state.data.join("scripts").to_string_lossy(), None::<&str>)?,
        "autostart" => {
            let launcher = app.autolaunch();
            if launcher.is_enabled()? { launcher.disable()?; } else { launcher.enable()?; }
            refresh(app)?;
        }
        _ => {
            if let Some(language) = id.strip_prefix("language:") {
                if !["en", "zh-CN"].contains(&language) { return Err("Unsupported language".into()); }
                let preferences = Preferences { language: language.into() };
                save_preferences(&state, &preferences)?;
                *state.preferences.lock().unwrap() = preferences;
                refresh(app)?;
            } else {
                let script = state.scripts.lock().unwrap().get(id).cloned();
                if let Some(script) = script { run_script(app, script)?; }
            }
        }
    }
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_, _, _| {}))
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data = dirs::home_dir().ok_or("Unable to locate home directory")?.join(".nookits");
            let scripts = data.join("scripts");
            initialize_scripts(&scripts, &app.path().resource_dir()?.join("examples"))?;
            let preferences_path = data.join("settings.json");
            let preferences = if preferences_path.exists() {
                let preferences: Preferences = serde_json::from_slice(&fs::read(&preferences_path)?)?;
                if !["en", "zh-CN"].contains(&preferences.language.as_str()) { return Err("Unsupported saved language".into()); }
                preferences
            } else { Preferences { language: "zh-CN".into() } };
            app.manage(State { data, preferences: Mutex::new(preferences.clone()), running: Mutex::new(HashSet::new()), children: Mutex::new(Vec::new()), scripts: Mutex::new(HashMap::new()) });
            if !preferences_path.exists() {
                save_preferences(&app.state::<State>(), &preferences)
                    .map_err(|error| -> Box<dyn std::error::Error> { error })?;
            }
            TrayIconBuilder::with_id("main")
                .icon(tauri::image::Image::from_bytes(include_bytes!("../generated/icons/icon.ico"))?)
                .tooltip("Nookits").show_menu_on_left_click(true)
                .on_menu_event(|app, event| {
                    if let Err(error) = handle_menu(app, event.id.as_ref()) {
                        eprintln!("{error}"); message(app, "error"); let _ = refresh(app);
                    }
                }).build(app)?;
            refresh(app.handle())
                .map_err(|error| -> Box<dyn std::error::Error> { error })?;
            let handle = app.handle().clone();
            let (sender, receiver) = std::sync::mpsc::channel();
            let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                if event.is_ok() { let _ = sender.send(()); }
            })?;
            watcher.watch(&scripts, notify::RecursiveMode::NonRecursive)?;
            std::thread::spawn(move || {
                let _watcher = watcher;
                while receiver.recv().is_ok() {
                    std::thread::sleep(Duration::from_millis(250));
                    while receiver.try_recv().is_ok() {}
                    let app = handle.clone();
                    let _ = handle.run_on_main_thread(move || { if let Err(error) = refresh(&app) { eprintln!("{error}"); } });
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Unable to start Nookits");
}
