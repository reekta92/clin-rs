use crate::app::App;
use crate::cli::PropertyCmd;
use crate::frontmatter::{self, FrontmatterEdit};
use crate::property_model::{
    PropertyDefinitions, PropertyKind, parse_property_value, property_edit,
};
use anyhow::{Context, Result, ensure};
use clap::ValueEnum;

pub(crate) fn ready_app() -> Result<App> {
    let (storage, _) = crate::storage::Storage::init();
    let mut app = App::new(storage?)?;
    app.ensure_catalog_ready()?;
    Ok(app)
}
pub(crate) fn resolve_note(app: &App, selector: &str) -> Result<String> {
    if let Ok(path) = app.storage.property_note_path(selector)
        && path.is_file()
        && matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("md" | "txt" | "clin")
        )
    {
        return Ok(selector.into());
    }
    let mut notes = app
        .visible_notes()
        .filter(|(_, note)| note.title.eq_ignore_ascii_case(selector.trim()));
    let id = notes
        .next()
        .map(|(_, note)| note.id.clone())
        .context("No note found; use exact note ID or title")?;
    ensure!(
        notes.next().is_none(),
        "Ambiguous title; use vault-relative note ID"
    );
    Ok(id)
}
pub(crate) fn creation_edits(
    app: &App,
    arguments: &[String],
) -> Result<(Vec<FrontmatterEdit>, Option<PropertyDefinitions>)> {
    ensure!(
        app.property_definitions_error.is_none(),
        "Repair property definitions before creating typed properties"
    );
    let mut definitions = std::borrow::Cow::Borrowed(&app.property_definitions);
    let mut edits = Vec::new();
    let mut keys = std::collections::BTreeSet::new();
    let mut changed = false;
    let (chunks, remainder) = arguments.as_chunks::<3>();
    ensure!(remainder.is_empty(), "--property expects KEY TYPE VALUE");
    for chunk in chunks {
        ensure!(
            keys.insert(&chunk[0]),
            "Duplicate initial property {}",
            chunk[0]
        );
        let kind = PropertyKind::from_str(&chunk[1], false).map_err(anyhow::Error::msg)?;
        let value = parse_property_value(kind, &chunk[2])?;
        if let Some(definition) = crate::property_model::inferred_definition(
            &definitions,
            &app.notes,
            &chunk[0],
            kind,
            &value,
            &[],
        )? {
            definitions
                .to_mut()
                .properties
                .insert(chunk[0].clone(), definition);
            changed = true;
        }
        edits.push(property_edit(&chunk[0], Some(&value))?);
    }
    Ok((edits, changed.then(|| definitions.into_owned())))
}
pub(crate) fn create(
    app: &mut App,
    template_name: Option<&str>,
    title: Option<String>,
    body: Option<String>,
    properties: &[String],
    fallback_title: &str,
) -> Result<(String, String)> {
    let mut template = if let Some(name) = template_name {
        let summary = app
            .storage
            .list_templates()?
            .into_iter()
            .find(|template| template.name == name || template.filename == name)
            .with_context(|| format!("Template not found: {name}"))?;
        app.storage.load_template(&summary.filename)?
    } else {
        app.storage
            .load_default_template()
            .unwrap_or(crate::templates::Template {
                name: String::new(),
                title: crate::templates::TitleConfig::default(),
                content: crate::templates::ContentConfig::default(),
                properties: std::collections::BTreeMap::default(),
            })
    };
    if let Some(body) = body {
        template.content.template = body;
    }
    let (edits, definitions) = creation_edits(app, properties)?;
    let rendered = template.render(
        definitions.as_ref().unwrap_or(&app.property_definitions),
        &edits,
    )?;
    let title = title
        .or(rendered.title)
        .unwrap_or_else(|| fallback_title.into());
    let note = crate::storage::Note {
        title: title.clone(),
        content: rendered.content,
        updated_at: crate::ui::now_unix_secs(),
        tags: rendered.tags,
    };
    if let Some(definitions) = definitions {
        let created: Vec<_> = definitions
            .properties
            .keys()
            .filter(|key| !app.property_definitions.properties.contains_key(*key))
            .cloned()
            .collect();
        app.save_property_definitions(definitions)?;
        for key in created {
            report_definition(&key);
        }
    }
    let id = app.storage.new_note_id();
    let saved = app
        .storage
        .create_note_with_header(&id, &note, rendered.header.as_deref())?;
    app.enqueue_backup(format!("properties: create {title}"));
    Ok((saved, title))
}
fn report_definition(key: &str) {
    eprintln!(
        "Created vault definition {} in .clin/properties.toml; type applies vault-wide. Existing values are not rewritten.",
        crate::fsutil::sanitize_for_terminal(key)
    );
}
fn warn_metadata(id: &str) {
    if id.ends_with(".clin") {
        eprintln!(
            "Warning: encrypted note properties are plaintext YAML. Encryption and Git backup do not hide this metadata."
        );
    }
}
pub(crate) fn run(action: PropertyCmd) -> Result<()> {
    let mut app = ready_app()?;
    match action {
        PropertyCmd::List { note } => {
            let id = resolve_note(&app, &note)?;
            let header = app.storage.load_frontmatter(&id)?;
            let values = header
                .as_deref()
                .map(frontmatter::checked_parse)
                .transpose()?
                .unwrap_or_default();
            print!("{}", serde_yaml_ng::to_string(&values.extra)?);
        }
        PropertyCmd::Get { note, key } => {
            crate::property_model::validate_key(&key)?;
            let id = resolve_note(&app, &note)?;
            let header = app
                .storage
                .load_frontmatter(&id)?
                .context("Property is missing")?;
            let values = frontmatter::checked_parse(&header)?;
            let value = values
                .extra
                .get(serde_yaml_ng::Value::String(key))
                .context("Property is missing")?;
            print!("{}", serde_yaml_ng::to_string(value)?);
        }
        PropertyCmd::Set {
            note,
            key,
            value,
            kind,
        } => {
            ensure!(
                app.property_definitions_error.is_none(),
                "Repair property definitions before editing"
            );
            let id = resolve_note(&app, &note)?;
            let kind = kind
                .or_else(|| {
                    app.property_definitions
                        .properties
                        .get(&key)
                        .map(|definition| definition.kind)
                })
                .unwrap_or(PropertyKind::String);
            let value = parse_property_value(kind, &value)?;
            let edit = property_edit(&key, Some(&value))?;
            let header = app.storage.load_frontmatter(&id)?;
            frontmatter::apply_edits(
                header.as_deref().unwrap_or("---\n---\n"),
                std::slice::from_ref(&edit),
            )?;
            if let Some(definition) = crate::property_model::inferred_definition(
                &app.property_definitions,
                app.notes.iter().filter(|note| note.id != id),
                &key,
                kind,
                &value,
                &[],
            )? {
                let mut definitions = app.property_definitions.clone();
                definitions.properties.insert(key.clone(), definition);
                app.save_property_definitions(definitions)?;
                report_definition(&key);
            }
            warn_metadata(&id);
            app.storage.update_properties(&id, &[edit])?;
            app.enqueue_backup(format!("properties: set {key}"));
            println!("Updated {id}: {key}");
        }
        PropertyCmd::Unset { note, key } => {
            let id = resolve_note(&app, &note)?;
            let edit = property_edit(&key, None)?;
            warn_metadata(&id);
            app.storage.update_properties(&id, &[edit])?;
            app.enqueue_backup(format!("properties: unset {key}"));
            println!("Updated {id}: removed {key}");
        }
        PropertyCmd::Rename {
            old,
            new,
            note,
            all,
            apply,
        } => {
            let mut batch = if all {
                app.prepare_property_rename(&old, &new)?
            } else {
                let id = resolve_note(&app, note.as_deref().context("--note or --all required")?)?;
                let before = app.storage.load_frontmatter(&id)?;
                let after = frontmatter::rename_key(
                    before.as_deref().context("Property missing")?,
                    &old,
                    &new,
                )?;
                crate::property_management::PropertyBatch {
                    notes: vec![crate::property_management::HeaderChange { id, before, after }],
                    bindings: Vec::new(),
                    ..Default::default()
                }
            };
            print!("{}", toml::to_string_pretty(&batch)?);
            if apply {
                let failures = batch.apply(&mut app)?;
                ensure!(
                    failures.is_empty(),
                    "Partial rename; retry notes properties resume --apply:\n{}",
                    failures.join("\n")
                );
            } else {
                println!("Preview only; repeat with --apply to confirm.");
            }
        }
        PropertyCmd::Resume { apply } => {
            let path = app
                .storage
                .data_dir
                .join(".clin")
                .join("property_batch.toml");
            let mut batch: crate::property_management::PropertyBatch = toml::from_str(
                &std::fs::read_to_string(path).context("No pending property batch")?,
            )?;
            print!("{}", toml::to_string_pretty(&batch)?);
            if apply {
                let failures = batch.apply(&mut app)?;
                ensure!(
                    failures.is_empty(),
                    "Partial batch; pending changes retained:\n{}",
                    failures.join("\n")
                );
            } else {
                println!("Preview only; repeat with --apply to confirm.");
            }
        }
    }
    Ok(())
}
