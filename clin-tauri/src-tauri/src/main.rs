#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--web" || a == "-w") {
        clin_tauri_lib::run_web_server();
    } else {
        // Allow passing a folder path directly: `clin-tauri /path/to/vault`
        let vault_arg = args.iter().skip(1)
            .find(|a| !a.starts_with('-'))
            .and_then(|p| {
                let path = std::path::PathBuf::from(p);
                if path.is_dir() { Some(path) } else { None }
            });
        clin_tauri_lib::run_with_vault(vault_arg);
    }
}
