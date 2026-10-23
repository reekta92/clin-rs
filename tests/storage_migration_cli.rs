#![cfg(target_os = "linux")]

use std::{fs, process::Command};

#[test]
fn migration_preserves_current_vault_paths_and_converts_legacy_only_on_request()
-> anyhow::Result<()> {
    for legacy in [false, true] {
        let root = tempfile::tempdir()?;
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        let files = [
            "notes/a.md",
            ".clin/subnotes.bin",
            ".clin/document-recovery-v2/draft.bin",
            ".clin/templates/current.toml",
            ".obsidian/types.json",
        ];
        for id in files {
            let path = source.join(id);
            fs::create_dir_all(path.parent().unwrap())?;
            fs::write(path, id)?;
        }
        if legacy {
            fs::create_dir_all(source.join("templates"))?;
            fs::write(source.join("templates/legacy.toml"), "legacy template")?;
        }
        let config = root.path().join("config.toml");
        fs::write(
            &config,
            format!(
                "[core]\nstorage_path = {}\n",
                serde_json::to_string(&destination)?
            ),
        )?;
        let data = root.path().join("data/clin");
        fs::create_dir_all(&data)?;
        fs::write(
            data.join("state.json"),
            serde_json::to_vec(&serde_json::json!({
                "version": 1,
                "storage_migration": {"previous_path": source, "target_path": destination}
            }))?,
        )?;
        let mut command = Command::new(env!("CARGO_BIN_EXE_clin"));
        command
            .arg("--config")
            .arg(&config)
            .args(["storage", "migrate"])
            .env("XDG_DATA_HOME", root.path().join("data"))
            .env("XDG_CONFIG_HOME", root.path().join("config"))
            .env("XDG_CACHE_HOME", root.path().join("cache"))
            .stdin(std::process::Stdio::null());
        if legacy {
            command.arg("--legacy-layout");
        }
        let output = command.output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        for id in files {
            let target = if legacy && id == "notes/a.md" {
                "a.md"
            } else {
                id
            };
            assert_eq!(fs::read_to_string(destination.join(target))?, id);
            assert_eq!(fs::read_to_string(source.join(id))?, id);
        }
        assert!(
            !destination
                .join(if legacy { "notes/a.md" } else { "a.md" })
                .exists()
        );
        if legacy {
            assert_eq!(
                fs::read_to_string(destination.join(".clin/templates/legacy.toml"))?,
                "legacy template"
            );
        }
        let state: serde_json::Value = serde_json::from_slice(&fs::read(data.join("state.json"))?)?;
        assert!(state.get("storage_migration").is_none());
    }
    Ok(())
}
