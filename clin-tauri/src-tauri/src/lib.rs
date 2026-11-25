mod commands;
use commands::*;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    Manager,
};

pub struct AppState {
    pub vault_path: Mutex<PathBuf>,
}

// ---------------------------------------------------------------------------
// Embedded HTTP API server (port 7779)
//
// Runs in a background thread inside the Tauri process.  The JS frontend
// fetches http://127.0.0.1:7779/<endpoint> — plain HTTP, no WebKit scheme
// restrictions, no __TAURI_INTERNALS__ timing races.
// ---------------------------------------------------------------------------

pub const API_PORT: u16 = 7779;

fn start_api_server(vault: Arc<Mutex<PathBuf>>) {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = match TcpListener::bind(format!("127.0.0.1:{}", API_PORT)) {
        Ok(l)  => { eprintln!("[clin] API server on http://127.0.0.1:{}", API_PORT); l }
        Err(e) => { eprintln!("[clin] WARNING: cannot start API server: {}", e); return; }
    };

    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let vault_path = vault.lock().unwrap().clone();

        // Read the full request (up to 1 MiB for large saves)
        let mut buf = vec![0u8; 1024 * 1024];
        let n = match stream.read(&mut buf) { Ok(n) => n, Err(_) => continue };
        let raw = String::from_utf8_lossy(&buf[..n]);

        // Parse first line: METHOD /path?qs HTTP/1.x
        let first = raw.lines().next().unwrap_or("");
        let mut parts = first.splitn(3, ' ');
        let method = parts.next().unwrap_or("GET");
        let full_path = parts.next().unwrap_or("/");

        // OPTIONS preflight
        if method == "OPTIONS" {
            let _ = stream.write_all(
                b"HTTP/1.1 204 No Content\r\n\
                  Access-Control-Allow-Origin: *\r\n\
                  Access-Control-Allow-Headers: Content-Type\r\n\
                  Access-Control-Allow-Methods: GET, POST, DELETE, OPTIONS\r\n\
                  Content-Length: 0\r\n\r\n"
            );
            continue;
        }

        let (path, qs) = match full_path.find('?') {
            Some(i) => (&full_path[..i], &full_path[i+1..]),
            None    => (full_path, ""),
        };

        eprintln!("[clin-api] {} {}", method, full_path);

        // Body follows the blank line separating headers from body
        let body_json: serde_json::Value = if let Some(idx) = raw.find("\r\n\r\n") {
            let body_str = raw[idx+4..].trim_end_matches('\0');
            serde_json::from_str(body_str).unwrap_or(serde_json::Value::Null)
        } else {
            serde_json::Value::Null
        };

        let (status, resp) = commands::handle_proto_request(
            &vault_path, method, path, qs, &body_json,
        );

        // If vault was switched, persist the change for future requests
        if method == "POST" && path == "/settings/vault" && status == 200 {
            if let Some(np) = body_json.get("path").and_then(|v| v.as_str()) {
                let p = PathBuf::from(np);
                if p.exists() { *vault.lock().unwrap() = p; }
            }
        }

        let body_bytes = serde_json::to_vec(&resp).unwrap_or_default();
        let header = format!(
            "HTTP/1.1 {} \r\n\
             Content-Type: application/json\r\n\
             Access-Control-Allow-Origin: *\r\n\
             Access-Control-Allow-Headers: Content-Type\r\n\
             Access-Control-Allow-Methods: GET, POST, DELETE, OPTIONS\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\r\n",
            status, body_bytes.len()
        );
        let _ = stream.write_all(header.as_bytes());
        let _ = stream.write_all(&body_bytes);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    run_with_vault(None);
}

pub fn run_with_vault(vault_override: Option<PathBuf>) {
    let initial_vault = vault_override
        .or_else(|| std::env::var("CLIN_VAULT").ok().map(PathBuf::from))
        .or_else(|| commands::saved_vault_path())
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("vault")
        });

    // Start the embedded HTTP API server in a background thread
    let shared_vault = Arc::new(Mutex::new(initial_vault.clone()));
    {
        let sv = Arc::clone(&shared_vault);
        std::thread::spawn(move || start_api_server(sv));
    }

    tauri::Builder::default()
        // ── Tauri-native plugins ─────────────────────────────────────────────
        // Persist window size, position, and maximized state across sessions.
        .plugin(tauri_plugin_window_state::Builder::default().build())
        // Native file/folder picker dialogs (used by pick_vault_dir command).
        .plugin(tauri_plugin_dialog::init())
        // OS-level notifications for CalDAV sync results, errors, etc.
        .plugin(tauri_plugin_notification::init())
        // Global keyboard shortcut to show/hide the window from anywhere.
        .plugin(tauri_plugin_global_shortcut::Builder::default().build())
        // Clipboard: copy note as Markdown or rendered HTML.
        .plugin(tauri_plugin_clipboard_manager::init())
        // Shell: open files/URLs in native OS applications.
        .plugin(tauri_plugin_shell::init())
        // ── App state ────────────────────────────────────────────────────────
        .manage({
            eprintln!("[clin] starting — vault={}", initial_vault.display());
            AppState { vault_path: Mutex::new(initial_vault.clone()) }
        })
        // ── Custom URI scheme (clin://) ──────────────────────────────────────
        // JS calls fetch("clin://localhost/endpoint") instead of invoke().
        // This bypasses the __TAURI_INTERNALS__ IPC path entirely and works
        // in both devUrl and production modes.
        .register_uri_scheme_protocol("clin", |app, request| {
            let vault = app
                .app_handle()
                .state::<AppState>()
                .vault_path
                .lock()
                .unwrap()
                .clone();

            let uri  = request.uri();
            let path = uri.path().to_string();
            let qs   = uri.query().unwrap_or("").to_string();
            let meth = request.method().as_str().to_string();

            let req_body: serde_json::Value = if !request.body().is_empty() {
                serde_json::from_slice(request.body()).unwrap_or(serde_json::Value::Null)
            } else {
                serde_json::Value::Null
            };

            eprintln!("[clin-proto] {} {}{}", meth, path,
                if qs.is_empty() { String::new() } else { format!("?{}", qs) });

            let (status, resp_body) =
                commands::handle_proto_request(&vault, &meth, &path, &qs, &req_body);

            // set_vault also updates the in-memory vault path so the next
            // request uses the new vault without requiring a restart.
            if meth == "POST" && path == "/settings/vault" && status == 200 {
                if let Some(new_path) = req_body.get("path").and_then(|v| v.as_str()) {
                    let pth = PathBuf::from(new_path);
                    if pth.exists() {
                        if let Ok(mut g) = app.app_handle().state::<AppState>().vault_path.lock() {
                            *g = pth;
                        }
                    }
                }
            }

            let bytes = serde_json::to_vec(&resp_body).unwrap_or_default();

            tauri::http::Response::builder()
                .status(status)
                .header("Content-Type", "application/json")
                .header("Access-Control-Allow-Origin", "*")
                .header("Access-Control-Allow-Headers", "Content-Type")
                .header("Access-Control-Allow-Methods", "GET, POST, DELETE, OPTIONS")
                .body(bytes)
                .unwrap()
        })
        // ── Commands ─────────────────────────────────────────────────────────
        .invoke_handler(tauri::generate_handler![
            get_graph,
            get_tree,
            get_file,
            get_notes,
            get_pinned_notes,
            get_calendar,
            get_properties,
            get_outline,
            get_subnotes,
            get_bases,
            get_base,
            get_canvas_files,
            get_canvas,
            get_themes,
            get_vaults,
            get_templates,
            open_file,
            save_file,
            create_file,
            delete_file,
            delete_folder,
            duplicate_file,
            rename_file,
            move_file,
            create_folder,
            create_link,
            delete_link,
            pin_note,
            save_properties,
            search_notes,
            get_smart_folder,
            set_theme,
            set_vault,
            pick_vault_dir,
            sync_calendar,
            get_health,
            get_settings,
            set_settings,
            get_recent_vaults,
        ])
        // ── Setup: system tray + global shortcut ─────────────────────────────
        .setup(|app| {
            // System tray with show/hide/quit menu
            let show = MenuItem::with_id(app, "show", "Show clin", true, None::<&str>)?;
            let hide = MenuItem::with_id(app, "hide", "Hide clin", true, None::<&str>)?;
            let sep  = tauri::menu::PredefinedMenuItem::separator(app)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &hide, &sep, &quit])?;

            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, .. } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "hide" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.hide();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            // Global shortcut: Ctrl+Shift+C to toggle clin window visibility
            use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};
            let shortcut = Shortcut::new(
                Some(Modifiers::CONTROL | Modifiers::SHIFT),
                Code::KeyC,
            );
            app.global_shortcut().on_shortcut(shortcut, move |app, _shortcut, _event| {
                if let Some(w) = app.get_webview_window("main") {
                    if w.is_visible().unwrap_or(false) {
                        let _ = w.hide();
                    } else {
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                }
            })?;

            // Inject API base URL and auto-open devtools in debug builds
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.eval(&format!(
                    "window.__CLIN_API_URL__ = 'http://127.0.0.1:{}'",
                    API_PORT
                ));
                #[cfg(debug_assertions)]
                w.open_devtools();
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running clin Tauri application");
}

// ---------------------------------------------------------------------------
// Web-server mode: `clin-tauri --web`
//
// Starts a minimal HTTP server on 127.0.0.1:7778 that serves the embedded
// clin_webview.html, then opens it in the system default browser.
// No Tauri window is created — useful for browser-only or headless use.
// ---------------------------------------------------------------------------

const WEBVIEW_HTML: &str = include_str!("../../clin_webview.html");

pub fn run_web_server() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let host = "127.0.0.1";
    let port: u16 = std::env::var("CLIN_WEB_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(7778);

    let addr = format!("{}:{}", host, port);
    let listener = TcpListener::bind(&addr)
        .unwrap_or_else(|e| panic!("Cannot bind to {}: {}", addr, e));

    let url = format!("http://{}:{}", host, port);
    eprintln!("[clin] web mode — serving at {}", url);
    eprintln!("[clin] press Ctrl+C to stop");

    // Open the browser (best-effort; ignore errors on headless systems)
    let _ = open_browser(&url);

    for stream in listener.incoming() {
        if let Ok(mut stream) = stream {
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let req = String::from_utf8_lossy(&buf);
            let first_line = req.lines().next().unwrap_or("");

            let (status, content_type, body): (&str, &str, &[u8]) =
                if first_line.starts_with("GET / ")
                    || first_line.starts_with("GET /index.html ")
                {
                    ("200 OK", "text/html; charset=utf-8", WEBVIEW_HTML.as_bytes())
                } else {
                    ("404 Not Found", "text/plain", b"Not Found")
                };

            let response = format!(
                "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
                status,
                content_type,
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(body);
        }
    }
}

fn open_browser(url: &str) {
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .args(["/c", "start", "", url])
        .spawn();
}
