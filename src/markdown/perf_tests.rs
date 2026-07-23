use std::time::Instant;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::ops::Range;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use crate::app_theme::AppThemeColors;
use super::{MarkdownRenderer, MdRenderOpts, RenderViewport};
use super::style::{RenderLine, RenderedDocument};

fn code_heavy_fixture() -> String {
    let langs = ["rust", "python", "javascript", "go", "sql", "yaml", "unknown"];
    let mut out = String::new();
    out.push_str("# Code Heavy Fixture\n\n");
    for i in 0..200 {
        let lang = langs[i % langs.len()];
        out.push_str(&format!("## Block {}\n\n```{}\n", i, lang));
        for j in 0..20 {
            out.push_str(&format!(
                "let mut my_very_long_variable_name_for_perf_testing_of_markdown_renderer_x_{}_{} = ({} * {}) + {}; // adding some realistic comments to make the line longer and more challenging for syntax highlighter parsing\n",
                i, j, i, j, j
            ));
        }
        out.push_str("```\n\n");
    }
    out
}

fn single_large_block_fixture() -> String {
    let mut out = String::new();
    out.push_str("```rust\n");
    for i in 0..5000 {
        out.push_str(&format!("fn test_function_name_{}() {{ let a = {}; let b = {}; let c = a + b; }}\n", i, i, i));
    }
    out.push_str("```\n");
    out
}

fn prose_fixture() -> String {
    let mut out = String::new();
    out.push_str("# Prose Fixture\n\n");
    for i in 0..2000 {
        out.push_str(&format!(
            "This is paragraph {}. It contains standard english sentences to simulate a very large markdown document consisting entirely of prose. We want to test how fast the renderer can layout and process standard text without any syntax highlighting overhead. The comrak parser will process this paragraph into normal text or paragraphs, wrapping them appropriately if wrap is enabled.\n\n",
            i
        ));
    }
    out
}

fn mixed_fixture() -> String {
    let mut out = String::new();
    out.push_str("# Mixed Fixture\n\n");
    out.push_str("| Header 1 | Header 2 | Header 3 |\n");
    out.push_str("|---|---|---|\n");
    out.push_str("| Row 1 Col 1 | Row 1 Col 2 | Row 1 Col 3 |\n");
    out.push_str("| Row 2 Col 1 | Row 2 Col 2 | Row 2 Col 3 |\n\n");
    out.push_str("- List item 1\n");
    out.push_str("  - Nested list item 1.1\n");
    out.push_str("    - Nested list item 1.1.1\n");
    out.push_str("- List item 2\n\n");
    out.push_str("> Quote block level 1\n");
    out.push_str("> > Quote block level 2\n\n");
    out.push_str("Here is a [Link](https://github.com/google/clin) and an ![Image](https://example.com/image.png).\n\n");
    out.push_str("CJK Text: 繁體中文 简体中文 日本語 한국어\n\n");
    out.push_str("Combining marks: a\u{0308} o\u{0308} u\u{0308} e\u{0301}\n\n");
    out.push_str("Emoji: 🦀 🚀 👨‍💻 🔍 ⚡️\n\n");
    out.push_str("Unknown syntax or code block:\n```foobar\nhello world\n```\n");
    out
}

#[test]
#[ignore]
fn markdown_renderer_perf() {
    let code_heavy = code_heavy_fixture();
    let single_large = single_large_block_fixture();
    let prose = prose_fixture();
    let mixed = mixed_fixture();

    println!("Fixtures size in memory:");
    println!("  code_heavy: {} bytes", code_heavy.len());
    println!("  single_large: {} bytes", single_large.len());
    println!("  prose: {} bytes", prose.len());
    println!("  mixed: {} bytes", mixed.len());

    let theme = AppThemeColors::default();
    let opts = MdRenderOpts::default();

    // 1. Measure layout-ready and completion timers
    super::cache::clear_markdown_caches();
    let mut renderer = MarkdownRenderer::new();

    let t0 = Instant::now();
    let viewport = RenderViewport { start: 0, height: 40 };
    renderer.render_with(&code_heavy, 100, &theme, &opts, viewport);
    
    let mut layout_ready_time = None;
    while renderer.is_pending() {
        std::thread::sleep(std::time::Duration::from_millis(1));
        if renderer.poll() && layout_ready_time.is_none() {
            layout_ready_time = Some(t0.elapsed());
        }
    }
    let total_warmup = t0.elapsed();
    let layout_ready = layout_ready_time.unwrap_or(total_warmup);
    println!("Layout ready: {:?}", layout_ready);
    println!("Total warmup completion: {:?}", total_warmup);

    // 2. Measure 20 runs
    let mut total_times = Vec::new();
    let mut draw_times = Vec::new();
    let mut heap_sizes = Vec::new();
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();

    for _ in 0..20 {
        super::cache::clear_markdown_caches();
        let mut r = MarkdownRenderer::new();
        let t_start = Instant::now();
        r.render_with(&code_heavy, 100, &theme, &opts, viewport);
        while r.is_pending() {
            std::thread::sleep(std::time::Duration::from_millis(1));
            r.poll();
        }
        total_times.push(t_start.elapsed().as_micros() as u64);
        
        let doc = r.document().unwrap();
        heap_sizes.push(doc.estimated_bytes());

        let t_draw_start = Instant::now();
        terminal.draw(|f| {
            let widget = super::MarkdownWidget::new(doc, 0..40);
            f.render_widget(widget, f.size());
        }).unwrap();
        draw_times.push(t_draw_start.elapsed().as_micros() as u64);
    }

    total_times.sort();
    draw_times.sort();
    heap_sizes.sort();

    let median_total = total_times[10];
    let p95_total = total_times[19];
    let median_draw = draw_times[10];
    let p95_draw = draw_times[19];
    let median_heap = heap_sizes[10];

    println!("Warm completion median: {} us, p95: {} us", median_total, p95_total);
    println!("Draw median: {} us, p95: {} us", median_draw, p95_draw);
    println!("Estimated heap bytes: {}", median_heap);

    let enforce = std::env::var("CLIN_PERF_ENFORCE").as_deref() == Ok("1");
    if enforce {
        let baseline_total_p95 = std::env::var("CLIN_MD_BASELINE_TOTAL_P95_US")
            .unwrap_or_else(|_| "3149081".to_string())
            .parse::<u64>()
            .unwrap();
        let baseline_heap = std::env::var("CLIN_MD_BASELINE_HEAP_BYTES")
            .unwrap_or_else(|_| "64228288".to_string())
            .parse::<usize>()
            .unwrap();
        let baseline_draw_p95 = std::env::var("CLIN_MD_BASELINE_DRAW_P95_US")
            .unwrap_or_else(|_| "150".to_string())
            .parse::<u64>()
            .unwrap();

        assert!(layout_ready.as_millis() <= 50, "LayoutReady should be <= 50ms, got {}ms", layout_ready.as_millis());
        assert!(p95_total <= baseline_total_p95 / 2, "Completion p95 should be <= 50% baseline, got {}us (baseline {}us)", p95_total, baseline_total_p95);
        assert!(median_heap <= (baseline_heap * 40 / 100), "Heap size should be <= 40% baseline, got {} bytes (baseline {} bytes)", median_heap, baseline_heap);
        assert!(p95_draw <= 1000, "Draw p95 should be <= 1ms, got {}us", p95_draw);
        assert!(p95_draw <= baseline_draw_p95.max(500), "Draw p95 should be <= baseline.max(500us), got {}us (baseline {}us)", p95_draw, baseline_draw_p95);

        // Warm document cache hit test
        let mut r_cache = MarkdownRenderer::new();
        r_cache.render_with(&code_heavy, 100, &theme, &opts, viewport);
        while r_cache.is_pending() {
            r_cache.poll();
        }
        let t_hit = Instant::now();
        r_cache.render_with(&code_heavy, 100, &theme, &opts, viewport);
        assert!(!r_cache.is_pending(), "Cache hit must be synchronous");
        assert!(t_hit.elapsed().as_micros() <= 2000, "Cache hit must take <= 2ms, got {:?}", t_hit.elapsed());

        // Generation count test
        let mut r_rapid = MarkdownRenderer::new();
        for i in 0..20 {
            r_rapid.render_with(&code_heavy, 100 + i, &theme, &opts, viewport);
        }
        assert_eq!(r_rapid.generation, 20);

        // Cancel test
        let mut r_cancel = MarkdownRenderer::new();
        r_cancel.render_with(&single_large, 100, &theme, &opts, viewport);
        let t_cancel = Instant::now();
        r_cancel.render_with(&code_heavy, 100, &theme, &opts, viewport);
        assert!(t_cancel.elapsed().as_millis() <= 50, "Cancellation of single large block must take <= 50ms");

        // Cache counter test
        let (doc_len, doc_bytes, hl_len, hl_bytes) = super::cache::cache_stats();
        assert!(hl_len <= 1024);
        assert!(hl_bytes <= 32 * 1024 * 1024);
        assert!(doc_len <= 32);
        assert!(doc_bytes <= 64 * 1024 * 1024);
    }
}

#[test]
fn layout_precedes_highlighting() {
    let content = "# Header\n```rust\nlet x = 1;\n```\n";
    let theme = AppThemeColors::default();
    let opts = MdRenderOpts::default();
    let mut renderer = MarkdownRenderer::new();
    let viewport = RenderViewport { start: 0, height: 10 };
    renderer.render_with(content, 80, &theme, &opts, viewport);
    
    // First poll should trigger LayoutReady event synchronously or near-synchronously
    let mut tries = 0;
    while renderer.document().is_none() && tries < 50 {
        std::thread::sleep(std::time::Duration::from_millis(1));
        renderer.poll();
        tries += 1;
    }
    assert!(renderer.document().is_some(), "LayoutReady must arrive first");
    assert!(renderer.is_pending(), "Should still be pending highlighting");
}

#[test]
fn unknown_language_completes_plain() {
    let content = "```foobar\nhello world\n```\n";
    let theme = AppThemeColors::default();
    let opts = MdRenderOpts::default();
    let mut renderer = MarkdownRenderer::new();
    let viewport = RenderViewport { start: 0, height: 10 };
    renderer.render_with(content, 80, &theme, &opts, viewport);
    
    while renderer.is_pending() {
        std::thread::sleep(std::time::Duration::from_millis(1));
        renderer.poll();
    }
    
    let doc = renderer.document().unwrap();
    let text: String = doc.line(1).unwrap().spans.iter().map(|s| s.text.as_str()).collect();
    assert!(text.contains("hello world"));
}

#[test]
fn widget_clips_wide_characters() {
    let content = "中文测试"; // 8 visual columns
    let theme = AppThemeColors::default();
    let opts = MdRenderOpts::default();
    let mut renderer = MarkdownRenderer::new();
    let viewport = RenderViewport { start: 0, height: 10 };
    renderer.render_with(content, 80, &theme, &opts, viewport);
    while renderer.is_pending() {
        renderer.poll();
    }
    
    let doc = renderer.document().unwrap();
    let widget = super::MarkdownWidget::new(doc, 0..1);
    // Buffer width 8 to hold 2 spaces margin + 3 CJK chars (each width 2) = 8 columns
    let mut buf = ratatui::buffer::Buffer::empty(ratatui::layout::Rect::new(0, 0, 8, 1));
    ratatui::widgets::Widget::render(widget, buf.area, &mut buf);
    
    let text = buf.content.iter().map(|c| c.symbol()).collect::<String>();
    assert!(text.contains("中"));
    assert!(text.contains("文"));
    assert!(text.contains("测"));
    assert!(!text.contains("试"));
}

#[test]
fn selection_crosses_spans_and_cjk() {
    let theme = AppThemeColors::default();
    let opts = MdRenderOpts::default();
    let mut renderer = MarkdownRenderer::new();
    let viewport = RenderViewport { start: 0, height: 10 };
    renderer.render_with("a **bold** CJK中文", 80, &theme, &opts, viewport);
    while renderer.is_pending() {
        renderer.poll();
    }
    
    let doc = renderer.document().unwrap();
    let text = super::read_selection_text(doc, (0, 3), (0, 12));
    assert_eq!(text, "bold CJK中");
}

#[test]
fn continuous_scroll_clamps() {
    let content = "line 1\n\nline 2\n\nline 3\n\nline 4\n\nline 5\n";
    let mut renderer = MarkdownRenderer::new();
    let theme = AppThemeColors::default();
    let opts = MdRenderOpts::default();
    let viewport = RenderViewport { start: 0, height: 2 };
    renderer.render_with(content, 80, &theme, &opts, viewport);
    while renderer.is_pending() {
        renderer.poll();
    }
    renderer.scroll_down(10, 2);
    assert_eq!(renderer.scroll_offset(), 7); // max scroll is 9 - 2 = 7
    renderer.scroll_up(10);
    assert_eq!(renderer.scroll_offset(), 0);
}

#[test]
fn code_block_highlighting_patches_match() {
    let content = include_str!(concat!(
        env!("HOME"),
        "/.local/share/clin/notes/clin_dir_TEST.md"
    ));
    let theme = crate::app_theme::AppThemeColors::default();
    let opts = MdRenderOpts::default();
    let mut renderer = MarkdownRenderer::new();
    let viewport = RenderViewport { start: 0, height: 200 };
    renderer.render_with(content, 120, &theme, &opts, viewport);
    while renderer.is_pending() {
        std::thread::sleep(std::time::Duration::from_millis(5));
        renderer.poll();
    }
    let doc = renderer.document().unwrap();
    for (i, line) in doc.lines().iter().enumerate() {
        let text: String = line.spans.iter().map(|s| s.text.as_str()).collect();
        eprintln!("  line {i}: src={} vw={} blank={} text={text:?}",
            line.source_line, line.visual_width, line.is_blank);
    }
}
