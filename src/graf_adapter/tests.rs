use super::*;

#[test]
fn duplicate_titles_keep_id_edges_and_disconnect_keeps_property_source() -> anyhow::Result<()> {
    let _guard = crate::config::ConfigTestGuard::lock();
    let dir = tempfile::tempdir()?;
    crate::config::set_config_path_override(dir.path().join("config.toml"));
    std::fs::create_dir_all(dir.path().join(".clin"))?;
    std::fs::write(
        dir.path().join(".clin/properties.toml"),
        "[properties.sources]\ntype = 'note_references'\n[properties.unknown]\ntype = 'note_reference'\n",
    )?;
    for (name, text) in [
        ("one.md", "---\ntitle: Duplicate\n---\nOne"),
        ("two.md", "---\ntitle: Duplicate\n---\nTwo"),
        (
            "source.md",
            "---\ntitle: Source\nsources: [one.md, two.md]\nunknown: Duplicate\n---\n[[one.md]] [[Duplicate]]",
        ),
    ] {
        std::fs::write(dir.path().join(name), text)?;
    }
    let storage = Storage {
        data_dir: dir.path().into(),
        notes_dir: dir.path().into(),
        config_dir: dir.path().into(),
        templates_dir: dir.path().join(".clin/templates"),
        key: [0; 32],
        skip_dir_patterns: Vec::new(),
        rename_on_title_change: false,
    };
    let notes = refresh_note_summaries(&storage);
    let mut config = ClinConfig::default();
    config.graf.filter.show_orphan = true;
    let specs = note_specs(&notes, &config.features);
    let graph = graf::build_graph(&specs, &clin_settings(&config))?;
    assert_eq!(graph.edge_count(), 2);
    let mut plugin = GrafPlugin::new(
        &config,
        storage.clone(),
        notes,
        Vec::new(),
        Keybinds::default(),
        crate::keybinds::KeyMatcher::new(),
    )?;
    assert_eq!(
        apply_connection(&mut plugin, "source.md", "one.md", false).as_deref(),
        Some("source.md")
    );
    let body = plugin.storage.load_note("source.md")?.content;
    assert!(!body.contains("[[one.md]]"));
    assert!(body.contains("[[Duplicate]]"));
    let header = plugin
        .storage
        .load_frontmatter("source.md")?
        .expect("header");
    assert!(header.contains("sources: [one.md, two.md]"));
    let guard = plugin.graph_state.as_ref().expect("graph").read();
    let graph = guard.simulation.get_graph();
    assert_eq!(graph.edge_count(), 2);
    drop(guard);
    let mut app = crate::app::App::new(storage)?;
    app.ensure_catalog_ready()?;
    app.load_and_open_note("source.md", None);
    let links = app.compute_links();
    assert!(
        links
            .iter()
            .any(|link| link.id == "one.md" && link.is_property && !link.is_body)
    );
    assert!(
        links
            .iter()
            .any(|link| link.unresolved && link.title == "Duplicate")
    );
    app.load_and_open_note("two.md", None);
    assert!(app.compute_links().iter().any(|link| link.id == "source.md"
        && link.is_backlink
        && link.is_property
        && !link.is_body));
    app.config.graf.preview_enabled = true;
    app.ensure_graph_preview();
    let preview = app
        .graph_preview
        .as_ref()
        .expect("preview")
        .simulation
        .get_graph();
    assert_eq!(preview.edge_count(), 2);
    Ok(())
}
