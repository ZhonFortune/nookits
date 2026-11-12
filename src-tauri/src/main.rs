#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{collections::{HashMap, HashSet}, fs::{self, OpenOptions}, io::{BufRead, BufReader, Read, Write}, path::{Path, PathBuf}, process::{Child, Command, Stdio}, sync::{Arc, Mutex}, time::Duration};
use notify::Watcher;
use serde::{Deserialize, Serialize};
use tauri::{menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu}, tray::TrayIconBuilder, AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use tauri_plugin_opener::OpenerExt;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
const INSTALLER_EXIT_ARGUMENT: &str = "--installer-exit";
const INSTALLER_REMOVE_USER_DATA_ARGUMENT: &str = "--installer-remove-user-data";
const DATA_DIRECTORY: &str = ".nookits";
const SCRIPTS_DIRECTORY: &str = "scripts";
const RUNTIME_LOG: &str = "runtime.log";

fn write_log(output: &mut impl Write, content: &str) -> std::io::Result<()> {
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f %:z");
    output.write_all(format!("[{timestamp}] {content}\n").as_bytes())
}

fn capture_output(input: impl Read + Send + 'static, log: Arc<Mutex<fs::File>>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        for line in BufReader::new(input).split(b'\n') {
            match line {
                Ok(line) => {
                    let text = String::from_utf8_lossy(&line);
                    let _ = write_log(&mut *log.lock().unwrap(), text.trim_end_matches('\r'));
                }
                Err(error) => {
                    let _ = write_log(&mut *log.lock().unwrap(), &format!("Unable to read script output: {error}"));
                    break;
                }
            }
        }
    })
}

fn remove_installer_user_data(profile: &Path) -> Result<()> {
    if !profile.is_absolute() || !profile.is_dir() {
        return Err("Invalid user profile directory".into());
    }
    let profile = fs::canonicalize(profile)?;
    let data = profile.join(DATA_DIRECTORY);
    let metadata = match fs::symlink_metadata(&data) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    #[cfg(windows)] {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("Refusing to remove a redirected user data directory".into());
        }
    }
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("Invalid user data directory".into());
    }
    fs::remove_dir_all(data)?;
    Ok(())
}

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
    let dialog = app.dialog().message(&text[key]).title("Nookits");
    if !["error", "scriptError"].contains(&key) {
        dialog.show(|_| {});
        return;
    }
    let log_path = state.data.join(RUNTIME_LOG);
    let handle = app.clone();
    dialog.buttons(MessageDialogButtons::OkCancelCustom(text["openLog"].clone(), text["close"].clone()))
        .show(move |open| {
            if !open { return; }
            let result = (|| -> Result<()> {
                OpenOptions::new().create(true).append(true).open(&log_path)?;
                handle.opener().open_path(log_path.to_string_lossy(), None::<&str>)?;
                Ok(())
            })();
            if let Err(error) = result {
                if let Ok(mut log) = OpenOptions::new().create(true).append(true).open(&log_path) {
                    let _ = write_log(&mut log, &error.to_string());
                }
                handle.dialog().message(&strings(&handle.state::<State>().preferences.lock().unwrap().language)["openLogError"])
                    .title("Nookits").show(|_| {});
            }
        });
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
    let mut paths = fs::read_dir(state.data.join(SCRIPTS_DIRECTORY))?
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
        let executable = dunce::canonicalize(executable)?;
        let runner = dunce::canonicalize(app.path().resource_dir()?.join("runtime/runtime.mjs"))?;
        let script_path = dunce::canonicalize(&script)?;
        let data = dunce::canonicalize(&state.data)?;
        let log_path = state.data.join(RUNTIME_LOG);
        if fs::metadata(&log_path).is_ok_and(|meta| meta.len() > 1_048_576) { fs::write(&log_path, b"")?; }
        let mut log = OpenOptions::new().create(true).append(true).open(log_path)?;
        write_log(&mut log, &format!("Running {}", script_path.display()))?;
        let mut command = Command::new(executable);
        command.arg(runner).arg(&script_path).arg(&data).current_dir(data.join(SCRIPTS_DIRECTORY))
            .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        #[cfg(windows)] {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        Ok((command.spawn()?, Arc::new(Mutex::new(log))))
    })();
    let (mut child, log) = match result {
        Ok(child) => child,
        Err(error) => { state.running.lock().unwrap().remove(&script); return Err(error); }
    };
    if let Some(pipe) = child.stdout.take() { capture_output(pipe, log.clone()); }
    if let Some(pipe) = child.stderr.take() { capture_output(pipe, log.clone()); }
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
        let _ = write_log(&mut *log.lock().unwrap(), &format!("Finished {}: {}", script.display(), if success { "success" } else { "failed or timed out" }));
        app.state::<State>().children.lock().unwrap().retain(|item| !Arc::ptr_eq(item, &child));
        app.state::<State>().running.lock().unwrap().remove(&script);
        if !success { message(&app, "scriptError"); }
    });
    Ok(())
}

fn quit(app: &AppHandle) {
    if let Some(state) = app.try_state::<State>() {
        for child in state.children.lock().unwrap().iter() {
            let mut child = child.lock().unwrap();
            let _ = child.kill();
            let _ = child.wait();
        }
    }
    app.exit(0);
}

fn handle_menu(app: &AppHandle, id: &str) -> Result<()> {
    let state = app.state::<State>();
    match id {
        "about" => show_about(app),
        "quit" => quit(app),
        "open" => app.opener().open_path(state.data.join(SCRIPTS_DIRECTORY).to_string_lossy(), None::<&str>)?,
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
    let mut arguments = std::env::args_os().skip(1);
    if arguments.next().is_some_and(|argument| argument == INSTALLER_REMOVE_USER_DATA_ARGUMENT) {
        let result = match (arguments.next(), arguments.next()) {
            (Some(profile), None) => remove_installer_user_data(Path::new(&profile)),
            _ => Err("Expected exactly one user profile directory".into()),
        };
        if let Err(error) = result {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, arguments, _| {
            if arguments.iter().skip(1).any(|argument| argument == INSTALLER_EXIT_ARGUMENT) {
                quit(app);
            }
        }))
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if std::env::args_os().skip(1).any(|argument| argument == INSTALLER_EXIT_ARGUMENT) {
                app.handle().exit(0);
                return Ok(());
            }
            let data = dirs::home_dir().ok_or("Unable to locate home directory")?.join(DATA_DIRECTORY);
            let scripts = data.join(SCRIPTS_DIRECTORY);
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
                        if let Ok(mut log) = OpenOptions::new().create(true).append(true).open(app.state::<State>().data.join(RUNTIME_LOG)) {
                            let _ = write_log(&mut log, &error.to_string());
                        }
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
