use super::eval::{Budget, Environment, Evaluator, Formulas};
use super::expr::Expression;
use super::format::BaseDocument;
use super::query::Query;
use super::value::{Text, Value, parse_date};
use crate::storage::{Storage, metadata::VaultMetadata};
use std::collections::BTreeMap;
use std::fs;
use std::sync::atomic::AtomicBool;

fn vault() -> (tempfile::TempDir, Storage) {
    let dir = tempfile::tempdir().unwrap();
    let storage = Storage {
        data_dir: dir.path().to_owned(),
        config_dir: dir.path().to_owned(),
        notes_dir: dir.path().to_owned(),
        templates_dir: dir.path().join("templates"),
        key: [0; 32],
        skip_dir_patterns: Vec::new(),
        rename_on_title_change: true,
    };
    (dir, storage)
}
fn metadata() -> (tempfile::TempDir, VaultMetadata) {
    let (dir, storage) = vault();
    fs::create_dir(dir.path().join("notes")).unwrap();
    fs::create_dir(dir.path().join(".obsidian")).unwrap();
    fs::write(
        dir.path().join(".obsidian/types.json"),
        r#"{"types":{"day":"date","timestamp":"datetime","textDay":"text"}}"#,
    )
    .unwrap();
    fs::write(dir.path().join("notes/a.md"),"---\nprice: 2\namount: 3\nstatus: Open\ndone: false\nauthor: '[[../host]]'\nauthors: ['[[../host|Alias]]', '[[../secret.clin]]']\ntags: [work/project]\nday: 2025-05-27\ntimestamp: 2025-05-27T12:30:45\ntextDay: 2025-05-27\nobj: {b: 2, a: 1}\n---\n[[../host]] [[../secret.clin]] ![[../cover.png]] #inline\n").unwrap();
    fs::write(
        dir.path().join("b.md"),
        "---\nprice: 5\namount: 2\nstatus: Done\ndone: true\n---\nBody\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("c.md"),
        "---\nprice: 5\namount: 1\n---\nBody\n",
    )
    .unwrap();
    fs::write(dir.path().join("host.md"), "---\nowner: Public\n---\nHost").unwrap();
    fs::write(
        dir.path().join("data.base"),
        BaseDocument::new_table().source(),
    )
    .unwrap();
    fs::write(dir.path().join("secret.clin"), "not valid ciphertext").unwrap();
    fs::write(dir.path().join(".hidden.md"), "private").unwrap();
    fs::write(dir.path().join("cover.png"), [0xff, 0]).unwrap();
    let metadata = storage
        .load_vault_metadata(&AtomicBool::new(false))
        .unwrap();
    (dir, metadata)
}
fn environment() -> Environment {
    Environment {
        host: Some("host.md".into()),
        now: parse_date("2025-05-30 12:30:45").unwrap(),
        random_seed: 42,
    }
}
fn evaluate(metadata: &VaultMetadata, source: &str) -> anyhow::Result<Value> {
    let expression = Expression::parse(source)?;
    let formulas = Formulas::default();
    let environment = environment();
    let cancelled = AtomicBool::new(false);
    let mut budget = Budget::new();
    Evaluator::new(
        metadata,
        &formulas,
        "notes/a.md",
        &environment,
        &cancelled,
        &mut budget,
    )
    .expression(&expression)
}
fn query(metadata: &VaultMetadata, source: &str) -> super::query::QueryResult {
    Query::compile(&BaseDocument::parse(source).unwrap(), 0)
        .unwrap()
        .execute(metadata, &environment(), &AtomicBool::new(false))
        .unwrap()
}

#[test]
fn numeric_operators_nullable_values_and_lazy_branches() {
    let (_dir, metadata) = metadata();
    for (source, expected) in [
        ("price * amount + 1", Value::Number(7.0)),
        ("10 - 3 - 2 * 2", Value::Number(3.0)),
        ("-5 % 2", Value::Number(-1.0)),
        ("(-2.5).round()", Value::Number(-2.0)),
        ("(2.5).toFixed(0)", Value::text("3")),
        ("(2.55).toFixed(1)", Value::text("2.5")),
        ("(1e21).toString()", Value::text("1e+21")),
        ("(-0).toString()", Value::text("0")),
        ("number('0xff')", Value::Number(255.0)),
        ("number(true)", Value::Number(1.0)),
        ("number(null)", Value::Number(0.0)),
        ("number('')", Value::Number(0.0)),
        ("if(false, number('invalid'))", Value::Null),
        ("true || number('invalid')", Value::Boolean(true)),
        ("false && number('invalid')", Value::Boolean(false)),
        ("missing * 5", Value::Null),
        ("missing > 1", Value::Boolean(false)),
        ("'2' == 2", Value::Boolean(false)),
        ("false.isEmpty()", Value::Boolean(false)),
        ("0.isEmpty()", Value::Boolean(false)),
        ("null.isEmpty()", Value::Boolean(true)),
        ("[].isTruthy()", Value::Boolean(true)),
    ] {
        assert_eq!(evaluate(&metadata, source).unwrap(), expected, "{source}");
    }
    assert!(evaluate(&metadata, "number('not a number')").is_err());
    assert!(evaluate(&metadata, "'x'.repeat(100000000)").is_err());
    assert!(evaluate(&metadata, "(2).toFixed(101)").is_err());
}

#[test]
fn strings_use_utf16_and_preserve_isolated_surrogates() {
    let (_dir, metadata) = metadata();
    for (source, expected) in [
        (r#"'😀'.length"#, Value::Number(2.0)),
        (r#"'😀'.slice(0,1)"#, Value::String(Text(vec![0xd83d]))),
        (
            r#"'😀'.reverse()"#,
            Value::String(Text(vec![0xde00, 0xd83d])),
        ),
        (
            r#"' a \uD800 b '.trim()"#,
            Value::String(Text(vec![97, 32, 0xd800, 32, 98])),
        ),
        (r#"'HELLO'.lower()"#, Value::text("hello")),
        (r#"'hello world'.title()"#, Value::text("Hello World")),
        (r#"'hello'.containsAll('he','lo')"#, Value::Boolean(true)),
        (r#"'hello'.containsAny('x','e')"#, Value::Boolean(true)),
        (
            r#"'hello'.startsWith('he') && 'hello'.endsWith('lo')"#,
            Value::Boolean(true),
        ),
        (r#"'hello'.slice(-3,-1)"#, Value::text("ll")),
        (r#"'abc'.repeat(0/0)"#, Value::text("")),
        (r#"'a:b:c'.replace(':','-')"#, Value::text("a-b-c")),
        (r#"'ab'.replace('','-')"#, Value::text("-a-b-")),
        (r#"'abc'.replace('b','[$&]')"#, Value::text("a[b]c")),
        (
            r#"'a,b,c'.split(',',2)"#,
            Value::List(vec![Value::text("a"), Value::text("b")]),
        ),
        (r#"'abc'.split('',1/0)"#, Value::List(Vec::new())),
        (
            r#"'😀'.split('')"#,
            Value::List(vec![
                Value::String(Text(vec![0xd83d])),
                Value::String(Text(vec![0xde00])),
            ]),
        ),
        (
            r#"escapeHTML('<a x="q">&')"#,
            Value::text("&lt;a x=&quot;q&quot;&gt;&amp;"),
        ),
    ] {
        assert_eq!(evaluate(&metadata, source).unwrap(), expected, "{source}");
    }
}

#[test]
fn list_iteration_scopes_reduce_nested_maps_and_objects() {
    let (_dir, metadata) = metadata();
    for (source, expected) in [
        (
            "[1,2,3].map(value + index)",
            Value::List(vec![
                Value::Number(1.0),
                Value::Number(3.0),
                Value::Number(5.0),
            ]),
        ),
        (
            "[1,2,3,4].filter(value > 2).reduce(acc + value,0)",
            Value::Number(7.0),
        ),
        (
            "[1,2].map([3,4].map(value + index).join('-')).join(',')",
            Value::text("3-5,3-5"),
        ),
        (
            "[1,2,3].reduce(if(value > acc,value,acc),0)",
            Value::Number(3.0),
        ),
        ("[].reduce(acc+value,7)", Value::Number(7.0)),
        (
            "[3,2,2,1].unique().sort().reverse().join('-')",
            Value::text("3-2-1"),
        ),
        ("[1,[2,3]].flat().slice(1).join(',')", Value::text("2,3")),
        ("[1,2,3].containsAll(1,3)", Value::Boolean(true)),
        ("[1,2,3].containsAny(8,2)", Value::Boolean(true)),
        ("[1,2,null].mean()", Value::Number(1.5)),
        ("{b: 2,a: 1}.keys().join(',')", Value::text("b,a")),
        ("obj.values().join(',')", Value::text("2,1")),
        ("{a: 1,a: 2}.a", Value::Number(2.0)),
        ("{}.isEmpty()", Value::Boolean(true)),
        ("list('one').length", Value::Number(1.0)),
        ("[0/0,0/0].unique().length", Value::Number(1.0)),
    ] {
        assert_eq!(evaluate(&metadata, source).unwrap(), expected, "{source}");
    }
}

#[test]
fn dates_durations_registry_types_and_moment_tokens() {
    let (_dir, metadata) = metadata();
    for (source, expected) in [
        (
            "day.isType('date') && timestamp.isType('date') && textDay.isType('string')",
            Value::Boolean(true),
        ),
        (
            "date('2024-01-31') + '1M'",
            Value::Date(parse_date("2024-02-29").unwrap()),
        ),
        (
            "(date('2024-12-01') + '1M' + '4h' + '3m').format('YYYY-MM-DD HH:mm:ss')",
            Value::text("2025-01-01 04:03:00"),
        ),
        (
            "(now() + duration('1d') * 2) - now()",
            Value::Number(172_800_000.0),
        ),
        ("(now() - '2h').hour", Value::Number(10.0)),
        ("now().date() == today()", Value::Boolean(true)),
        (
            "timestamp.year + timestamp.month + timestamp.day",
            Value::Number(2057.0),
        ),
        ("timestamp.time()", Value::text("12:30:45")),
        ("(now() - '3d').relative()", Value::text("3 days ago")),
        ("(now() + '3d').relative()", Value::text("in 3 days")),
        (
            "timestamp.format('[Date:] MMMM Do, YYYY [at] HH:mm:ss.SSS')",
            Value::text("Date: May 27th, 2025 at 12:30:45.000"),
        ),
        (
            "number(date('2025-05-27')) - number(date('2025-05-26'))",
            Value::Number(86_400_000.0),
        ),
    ] {
        assert_eq!(evaluate(&metadata, source).unwrap(), expected, "{source}");
    }
    for source in [
        "date('bad')",
        "duration('2z')",
        "now() + '1e999d'",
        "2 * duration('1d')",
        "now() + duration('1d') / 0",
    ] {
        assert!(evaluate(&metadata, source).is_err(), "{source}");
    }
}

#[test]
fn context_links_and_file_metadata_remain_public_only() {
    let (_dir, metadata) = metadata();
    for (source, expected) in [
        ("this.file.name", Value::text("host.md")),
        ("this.owner", Value::text("Public")),
        ("author == this", Value::Boolean(true)),
        ("authors.contains(this)", Value::Boolean(true)),
        ("file.hasLink(this.file)", Value::Boolean(true)),
        ("file.hasLink('secret.clin')", Value::Boolean(false)),
        ("file('secret.clin')", Value::Null),
        ("link('secret.clin').asFile()", Value::Null),
        ("file('.hidden.md')", Value::Null),
        ("file('/etc/passwd')", Value::Null),
        (
            "file.hasTag('work') && file.hasTag('#inline')",
            Value::Boolean(true),
        ),
        (
            "file.inFolder('notes') && !file.inFolder('note')",
            Value::Boolean(true),
        ),
        (
            "file.hasProperty('price') && !file.hasProperty('missing')",
            Value::Boolean(true),
        ),
        ("file.basename", Value::text("a")),
        ("file.ext", Value::text("md")),
        (
            "file.links.contains(this) && file.embeds.length == 1",
            Value::Boolean(true),
        ),
        (
            "file('host').backlinks.contains(file)",
            Value::Boolean(true),
        ),
        ("file.properties.obj.a", Value::Number(1.0)),
        (
            "link('missing', 'A') == link('missing', 'B')",
            Value::Boolean(true),
        ),
        (
            "link('../host#Header') == link('host.md')",
            Value::Boolean(true),
        ),
        ("link('../host').linksTo(file)", Value::Boolean(false)),
    ] {
        assert_eq!(evaluate(&metadata, source).unwrap(), expected, "{source}");
    }
    let forms = Formulas::default();
    let mut env = environment();
    env.host = Some("secret.clin".into());
    let cancelled = AtomicBool::new(false);
    let mut budget = Budget::new();
    let mut evaluator = Evaluator::new(
        &metadata,
        &forms,
        "notes/a.md",
        &env,
        &cancelled,
        &mut budget,
    );
    assert_eq!(
        evaluator
            .expression(&Expression::parse("this").unwrap())
            .unwrap(),
        Value::Null
    );
}

#[test]
fn native_regex_replace_split_captures_flags_and_utf16() {
    let (_dir, metadata) = metadata();
    for (source, expected) in [
        (r#"/abc/.matches('abcde')"#, Value::Boolean(true)),
        (r#"/abc/i.matches('ABC')"#, Value::Boolean(true)),
        (r#"/abc/y.matches('xabc')"#, Value::Boolean(false)),
        (r#"/^b/m.matches('a\nb')"#, Value::Boolean(true)),
        (r#"'a:b:c:d'.replace(/:/,'-')"#, Value::text("a-b:c:d")),
        (r#"'a:b:c:d'.replace(/:/g,'-')"#, Value::text("a-b-c-d")),
        (
            r#"'John Smith'.replace(/(\w+) (\w+)/,'$2, $1')"#,
            Value::text("Smith, John"),
        ),
        (r#"'a,b,c'.split(/,/,2).join('-')"#, Value::text("a-b")),
        (r#"'a1b2'.split(/(\d)/).join('-')"#, Value::text("a-1-b-2-")),
        (r#"'😀'.replace(/./g,'-')"#, Value::text("--")),
        (r#"'😀'.replace(/./gu,'-')"#, Value::text("-")),
        (r#"'\u{F0000}'.replace(/x/u,'-')"#, Value::text("\u{f0000}")),
        (r#"/(a+)\1/.matches('aaaa')"#, Value::Boolean(true)),
        (r#"/(?<=a)b/.matches('ab')"#, Value::Boolean(true)),
    ] {
        assert_eq!(evaluate(&metadata, source).unwrap(), expected, "{source}");
    }
    assert!(evaluate(&metadata, r#"/a/v.matches('a')"#).is_err());
    // Backtracking cannot stall the worker indefinitely on imported adversarial expressions.
    assert!(evaluate(&metadata, r#"/(a+)+\1$/.matches('a'.repeat(80)+'b')"#).is_err());
}

#[test]
fn formula_dependencies_cycles_missing_refs_and_dynamic_cycles() {
    let (_dir, metadata) = metadata();
    let definitions = BTreeMap::from([
        ("total".into(), "price * amount".into()),
        (
            "label".into(),
            "formula.total.toFixed(2) + ' dollars'".into(),
        ),
    ]);
    let formulas = Formulas::compile(&definitions).unwrap();
    let env = environment();
    let cancelled = AtomicBool::new(false);
    let mut budget = Budget::new();
    let mut evaluator = Evaluator::new(
        &metadata,
        &formulas,
        "notes/a.md",
        &env,
        &cancelled,
        &mut budget,
    );
    assert_eq!(
        evaluator.property("formula.label").unwrap(),
        Value::text("6.00 dollars")
    );
    for definitions in [
        BTreeMap::from([("a".into(), "formula.a".into())]),
        BTreeMap::from([
            ("a".into(), "formula.b".into()),
            ("b".into(), "formula.a".into()),
        ]),
        BTreeMap::from([("a".into(), "formula.missing".into())]),
    ] {
        assert!(Formulas::compile(&definitions).is_err());
    }
    let formulas =
        Formulas::compile(&BTreeMap::from([("a".into(), "formula['a' + '']".into())])).unwrap();
    let mut budget = Budget::new();
    let mut evaluator = Evaluator::new(
        &metadata,
        &formulas,
        "notes/a.md",
        &env,
        &cancelled,
        &mut budget,
    );
    assert!(evaluator.property("formula.a").is_err());
}

#[test]
fn filters_global_view_not_semantics_sorts_and_display_order_are_distinct() {
    let (_dir, metadata) = metadata();
    let result = query(
        &metadata,
        "filters: 'file.ext == \"md\"'\nviews:\n  - type: table\n    filters: {not: [done, 'price < 3']}\n    order: [note.amount, file.name]\n    sort: [{property: note.price, direction: DESC}, {property: note.amount, direction: ASC}]\n",
    );
    assert_eq!(
        result
            .rows
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        ["c.md", "host.md"]
    );
    assert_eq!(result.matched, 2);
    let result = query(
        &metadata,
        "filters: 'file.ext == \"md\" && price'\nformulas: {total: 'price * amount'}\nviews:\n  - type: table\n    order: [file.name, formula.total]\n    sort: [{property: note.price, direction: DESC}, {property: note.amount, direction: ASC}]\n",
    );
    assert_eq!(
        result
            .rows
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        ["c.md", "b.md", "notes/a.md"]
    );
}

#[test]
fn groups_manual_visibility_empty_groups_and_limits() {
    let (_dir, metadata) = metadata();
    let source = "filters: 'file.ext == \"md\" && price'\nviews:\n  - type: kanban\n    order: [file.name, note.status]\n    groupBy: {property: note.status, direction: DESC}\n    groupOrder: [Open, Future, null]\n";
    let result = query(&metadata, source);
    assert_eq!(result.matched, 3);
    assert_eq!(result.rows.len(), 2);
    assert_eq!(
        result
            .groups
            .iter()
            .map(|group| group.value.clone())
            .collect::<Vec<_>>(),
        [Value::text("Open"), Value::text("Future"), Value::Null]
    );
    assert_eq!(result.groups[1].rows.len(), 0);
    let result = query(&metadata, &source.replace("[Open, Future, null]", "[]"));
    assert!(result.groups.is_empty());
    assert!(result.rows.is_empty());
    assert_eq!(result.matched, 3);
    let result = query(
        &metadata,
        &source.replace("    groupOrder: [Open, Future, null]\n", ""),
    );
    assert_eq!(
        result
            .groups
            .iter()
            .map(|group| group.value.clone())
            .collect::<Vec<_>>(),
        [Value::text("Open"), Value::text("Done"), Value::Null]
    );
    let result = query(&metadata, &(source.to_owned() + "    limit: 1\n"));
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.matched, 3);
}

#[test]
fn summaries_builtin_custom_and_groups_use_visible_rows() {
    let (_dir, metadata) = metadata();
    let result = query(
        &metadata,
        "filters: 'file.ext == \"md\" && price'\nsummaries: {rounded: 'values.mean().round(3)'}\nviews:\n  - type: table\n    order: [file.name, note.price, note.amount, note.status, note.done]\n    groupBy: {property: note.status}\n    summaries: {note.price: rounded, note.amount: Sum, note.status: Unique, note.done: Checked}\n",
    );
    assert_eq!(result.summaries["note.price"], Value::Number(4.0));
    assert_eq!(result.summaries["note.amount"], Value::Number(6.0));
    assert_eq!(result.summaries["note.status"], Value::Number(3.0));
    assert_eq!(result.summaries["note.done"], Value::Number(1.0));
    let open = result
        .groups
        .iter()
        .find(|group| group.value == Value::text("Open"))
        .unwrap();
    assert_eq!(open.summaries["note.price"], Value::Number(2.0));
    for (name, expected) in [
        ("Average", 4.0),
        ("Sum", 12.0),
        ("Min", 2.0),
        ("Max", 5.0),
        ("Range", 3.0),
        ("Median", 5.0),
        ("Stddev", 2f64.sqrt()),
    ] {
        let source = format!(
            "filters: 'price'\nviews: [{{type: table, summaries: {{note.price: {name}}}}}]\n"
        );
        let result = query(&metadata, &source);
        let Value::Number(actual) = result.summaries["note.price"] else {
            panic!()
        };
        assert!((actual - expected).abs() < 1e-10, "{name}: {actual}");
    }
}

#[test]
fn unsupported_views_html_and_malformed_metadata_are_explicit() {
    let (_dir, metadata) = metadata();
    assert!(Query::compile(&BaseDocument::parse("views: [{type: map}]\n").unwrap(), 0).is_err());
    let result = query(
        &metadata,
        "filters: 'price'\nformulas: {content: 'html(\"<script>bad</script>\")'}\nviews: [{type: table, order: [formula.content]}]\n",
    );
    assert_eq!(result.diagnostics.len(), 1);
    assert!(matches!(
        result.rows[0].values["formula.content"],
        Value::Html(_)
    ));
    assert_eq!(metadata.len(), 6); // public notes, base, attachment; no ciphertext or hidden files
    let (dir, storage) = vault();
    fs::write(dir.path().join("bad.md"), "---\nx: [\n---\nBody").unwrap();
    let metadata = storage
        .load_vault_metadata(&AtomicBool::new(false))
        .unwrap();
    assert_eq!(
        query(&metadata, "views: [{type: table, order: [file.name]}]\n")
            .rows
            .len(),
        1
    );
    let query = Query::compile(
        &BaseDocument::parse("views: [{type: table, order: [note.x]}]\n").unwrap(),
        0,
    )
    .unwrap();
    let error = query
        .execute(&metadata, &environment(), &AtomicBool::new(false))
        .unwrap_err();
    assert!(format!("{error:#}").contains("bad.md"));
}

#[test]
fn cancellation_and_random_seed_lifecycle() {
    let (_dir, metadata) = metadata();
    let document = BaseDocument::parse(
        "formulas: {r: 'random()'}\nviews: [{type: table, order: [formula.r]}]\n",
    )
    .unwrap();
    let query = Query::compile(&document, 0).unwrap();
    assert!(
        query
            .execute(&metadata, &environment(), &AtomicBool::new(true))
            .is_err()
    );
    let env = environment();
    let first = query
        .execute(&metadata, &env, &AtomicBool::new(false))
        .unwrap();
    let second = query
        .execute(&metadata, &env, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(first.rows[0].values, second.rows[0].values);
    let mut next = env;
    next.random_seed = 1234;
    let next = query
        .execute(&metadata, &next, &AtomicBool::new(false))
        .unwrap();
    assert_ne!(first.rows[0].values, next.rows[0].values);
}

#[test]
#[ignore = "5,000-note release query benchmark; run with --release --ignored --nocapture"]
fn query_5000_notes_benchmark() {
    let (dir, storage) = vault();
    for index in 0..5000 {
        fs::write(dir.path().join(format!("note{index:04}.md")),format!("---\nprice: {}\namount: {}\nstatus: group{}\ntags: [work,bench/test]\n---\n[[note{:04}]]\n{}",index%50,index%7,index%20,(index+1)%5000,"Body. ".repeat(1700))).unwrap();
    }
    let metadata = storage
        .load_vault_metadata(&AtomicBool::new(false))
        .unwrap();
    let source = "filters: 'file.hasTag(\"work\")'\nformulas: {total: 'price * amount', label: 'file.basename + \" \" + formula.total.toFixed(2)'}\nsummaries: {custom: 'values.mean().round(2)'}\nviews:\n  - type: table\n    order: [file.name, note.price, note.amount, formula.total, formula.label]\n    sort: [{property: formula.total, direction: DESC}, {property: file.name}]\n    groupBy: {property: note.status}\n    summaries: {formula.total: custom, note.price: Sum}\n";
    let query = Query::compile(&BaseDocument::parse(source).unwrap(), 0).unwrap();
    let environment = environment();
    let cancelled = AtomicBool::new(false);
    let mut durations = Vec::new();
    for _ in 0..25 {
        let start = std::time::Instant::now();
        let result = query.execute(&metadata, &environment, &cancelled).unwrap();
        durations.push(start.elapsed());
        assert_eq!(result.rows.len(), 5000);
        assert_eq!(result.groups.len(), 20);
    }
    durations.sort();
    eprintln!(
        "5,000-note query p50 {:?}; p95 {:?}; max {:?}",
        durations[12], durations[23], durations[24]
    );
    assert!(
        durations[23] <= std::time::Duration::from_millis(150),
        "warm query p95 exceeds 150 ms"
    );
}
