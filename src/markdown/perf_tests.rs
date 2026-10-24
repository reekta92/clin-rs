use super::{MarkdownRenderer, MdRenderOpts, RenderViewport};
use crate::app_theme::AppThemeColors;

#[test]
fn layout_precedes_highlighting() {
    use super::{
        cache::RenderKey,
        style::MarkdownTheme,
        worker::{self, RenderEvent, RenderJob},
    };
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64},
        mpsc,
    };
    use std::time::Duration;

    let (tx, rx) = mpsc::sync_channel(0);
    worker::submit(RenderJob {
        generation: 1,
        key: RenderKey::new(
            Arc::from("# Header\n```rust\nlet x = 1;\n```\n"),
            80,
            MarkdownTheme::from_app_theme(&AppThemeColors::default()),
            MdRenderOpts::default(),
        ),
        viewport: Arc::new(AtomicU64::new(worker::pack_viewport(0, 10))),
        cancel: Arc::new(AtomicBool::new(false)),
        tx,
    });
    let timeout = Duration::from_secs(10);
    assert!(matches!(
        rx.recv_timeout(timeout).unwrap(),
        RenderEvent::LayoutReady { .. }
    ));
    assert!(matches!(
        rx.recv_timeout(timeout).unwrap(),
        RenderEvent::CodeBlockReady { .. }
    ));
    assert!(matches!(
        rx.recv_timeout(timeout).unwrap(),
        RenderEvent::Complete { .. }
    ));
}

#[test]
fn unknown_language_completes_plain() {
    let content = "```foobar\nhello world\n```\n";
    let theme = AppThemeColors::default();
    let opts = MdRenderOpts::default();
    let mut renderer = MarkdownRenderer::new();
    let viewport = RenderViewport {
        start: 0,
        height: 10,
    };
    renderer.render_with(content, 80, &theme, &opts, viewport);

    while renderer.is_pending() {
        std::thread::sleep(std::time::Duration::from_millis(1));
        renderer.poll();
    }

    let doc = renderer.document().unwrap();
    let text: String = doc
        .line(1)
        .unwrap()
        .spans
        .iter()
        .map(|s| s.text.as_str())
        .collect();
    assert!(text.contains("hello world"));
}

#[test]
fn widget_clips_wide_characters() {
    let content = "中文测试"; // 8 visual columns
    let theme = AppThemeColors::default();
    let opts = MdRenderOpts::default();
    let mut renderer = MarkdownRenderer::new();
    let viewport = RenderViewport {
        start: 0,
        height: 10,
    };
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
fn continuous_scroll_clamps() {
    let content = "line 1\n\nline 2\n\nline 3\n\nline 4\n\nline 5\n";
    let mut renderer = MarkdownRenderer::new();
    let theme = AppThemeColors::default();
    let opts = MdRenderOpts::default();
    let viewport = RenderViewport {
        start: 0,
        height: 2,
    };
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
    let content = "# Hello World\n\nSome paragraph text here.\n\n```rust\nfn main() {\n    println!(\"hello\");\n}\n```\n";
    let theme = crate::app_theme::AppThemeColors::default();
    let opts = MdRenderOpts::default();
    let mut renderer = MarkdownRenderer::new();
    let viewport = RenderViewport {
        start: 0,
        height: 200,
    };
    renderer.render_with(content, 120, &theme, &opts, viewport);
    while renderer.is_pending() {
        std::thread::sleep(std::time::Duration::from_millis(5));
        renderer.poll();
    }
    let doc = renderer.document().unwrap();
    for (i, line) in doc.lines().iter().enumerate() {
        let text: String = line.spans.iter().map(|s| s.text.as_str()).collect();
        eprintln!(
            "  line {i}: src={} vw={} blank={} text={text:?}",
            line.source_line, line.visual_width, line.is_blank
        );
    }
}

#[test]
fn suspended_preview_releases_workers_and_resumes_without_losing_scroll() {
    use std::fmt::Write;
    let content = (0..80).fold(String::new(), |mut content, index| {
        writeln!(content, "```text\nblock {index}\n```\n").unwrap();
        content
    });
    let theme = AppThemeColors::default();
    let opts = MdRenderOpts::default();
    let viewport = RenderViewport {
        start: 0,
        height: 20,
    };
    let mut hidden = MarkdownRenderer::new();
    hidden.render_with(&content, 80, &theme, &opts, viewport);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while hidden.document().is_none() {
        hidden.poll();
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    hidden.set_scroll_offset(10, 20);
    hidden.suspend();
    let mut active = MarkdownRenderer::new();
    active.render_with("# Active preview\n", 80, &theme, &opts, viewport);
    while active.is_pending() {
        active.poll();
        assert!(
            std::time::Instant::now() < deadline,
            "hidden preview blocked active rendering"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    while hidden.is_pending() {
        hidden.poll();
        assert!(
            std::time::Instant::now() < deadline,
            "suspended preview did not resume"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(hidden.scroll_offset(), 10);
    assert!(hidden.document().unwrap().line_count() > 80);
}
