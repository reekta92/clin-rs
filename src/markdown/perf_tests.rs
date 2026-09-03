use super::{MarkdownRenderer, MdRenderOpts, RenderViewport};
use crate::app_theme::AppThemeColors;

#[test]
fn layout_precedes_highlighting() {
    let content = "# Header\n```rust\nlet x = 1;\n```\n";
    let theme = AppThemeColors::default();
    let opts = MdRenderOpts::default();
    let mut renderer = MarkdownRenderer::new();
    let viewport = RenderViewport {
        start: 0,
        height: 10,
    };
    renderer.render_with(content, 80, &theme, &opts, viewport);

    // First poll should trigger LayoutReady event synchronously or near-synchronously
    let mut tries = 0;
    while renderer.document().is_none() && tries < 50 {
        std::thread::sleep(std::time::Duration::from_millis(1));
        renderer.poll();
        tries += 1;
    }
    assert!(
        renderer.document().is_some(),
        "LayoutReady must arrive first"
    );
    assert!(
        renderer.is_pending(),
        "Should still be pending highlighting"
    );
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
