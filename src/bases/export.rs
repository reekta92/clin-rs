//! Plain-data exports of visible public results. No vault reads or executable content.

use super::{query::QueryResult, value::Value};
use anyhow::{Context, Result, bail};
const MAX_EXPORT_BYTES: usize = 16 * 1024 * 1024;
fn cell(value: Option<&Value>, spreadsheet: bool) -> String {
    let Some(value) = value else {
        return String::new();
    };
    if matches!(value, Value::Null) {
        return String::new();
    }
    let text = value.to_text().display();
    // CSV quoting does not stop spreadsheet formula execution on untrusted text.
    if spreadsheet
        && !matches!(value, Value::Number(_) | Value::Boolean(_))
        && text.trim_start().starts_with(['=', '+', '-', '@'])
    {
        format!("'{text}")
    } else {
        text
    }
}
pub fn export(columns: &[String], result: &QueryResult, markdown: bool) -> Result<String> {
    if markdown {
        let escape = |text: String| {
            text.replace('\\', "\\\\")
                .replace('|', "\\|")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace(['\r', '\n'], " ")
        };
        let mut output = format!(
            "| {} |\n| {} |\n",
            columns
                .iter()
                .map(|column| escape(column.clone()))
                .collect::<Vec<_>>()
                .join(" | "),
            columns
                .iter()
                .map(|_| "---")
                .collect::<Vec<_>>()
                .join(" | ")
        );
        for row in &result.rows {
            output.push_str(&format!(
                "| {} |\n",
                columns
                    .iter()
                    .map(|column| escape(cell(row.values.get(column), false)))
                    .collect::<Vec<_>>()
                    .join(" | ")
            ));
            if output.len() > MAX_EXPORT_BYTES {
                bail!("export exceeds 16 MiB safety limit; no file created");
            }
        }
        Ok(output)
    } else {
        let mut writer = csv::WriterBuilder::new().from_writer(Vec::new());
        writer.write_record(columns)?;
        for row in &result.rows {
            writer.write_record(
                columns
                    .iter()
                    .map(|column| cell(row.values.get(column), true)),
            )?;
            writer.flush()?;
            if writer.get_ref().len() > MAX_EXPORT_BYTES {
                bail!("export exceeds 16 MiB safety limit; no file created");
            }
        }
        String::from_utf8(writer.into_inner().map_err(|error| error.into_error())?)
            .context("export encoding failed")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::bases::query::Row;
    #[test]
    fn csv_and_markdown_quote_untrusted_public_values_without_executing_them() {
        let result = QueryResult {
            rows: vec![Row {
                id: "public.md".into(),
                values: [
                    ("text".into(), Value::text("=cmd(),comma\nnext | <script>")),
                    ("number".into(), Value::Number(-2.0)),
                ]
                .into(),
            }],
            groups: Vec::new(),
            summaries: std::collections::BTreeMap::default(),
            matched: 1,
            diagnostics: Vec::new(),
        };
        let columns = vec!["text".into(), "number".into()];
        let csv = export(&columns, &result, false).unwrap();
        let record = csv::Reader::from_reader(csv.as_bytes())
            .records()
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(&record[0], "'=cmd(),comma\nnext | <script>");
        assert_eq!(&record[1], "-2");
        let md = export(&columns, &result, true).unwrap();
        assert!(md.contains("\\| &lt;script&gt;"));
        assert!(!md.contains("<script>"));
    }
}
