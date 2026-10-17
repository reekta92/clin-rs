use super::*;

#[test]
fn clipboard_notices_distinguish_native_success_transport_and_failure() {
    assert_eq!(
        clipboard_notice(Ok(ClipboardWrite::System)),
        "Copied to clipboard"
    );
    assert_eq!(
        clipboard_notice(Ok(ClipboardWrite::Terminal)),
        "Copy sent to terminal (OSC 52)"
    );
    assert_eq!(
        clipboard_notice(Err(anyhow::anyhow!("denied"))),
        "Copied internally; system clipboard unavailable"
    );
}

#[test]
fn osc52_encodes_unicode_and_control_bytes_as_data() {
    use base64::Engine;
    let text = "世界\n\x1b]52;c;untrusted\x07";
    let sequence = osc52_sequence(text, false).unwrap();
    let payload = sequence
        .strip_prefix("\x1b]52;c;")
        .unwrap()
        .strip_suffix('\x07')
        .unwrap();
    assert!(!payload.contains('\x1b'));
    assert!(!payload.contains('\x07'));
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(payload)
            .unwrap(),
        text.as_bytes()
    );
}

#[test]
fn tmux_passthrough_escapes_inner_escape_and_rejects_oversized_payloads() {
    assert_eq!(
        osc52_sequence("hello", true).unwrap(),
        "\x1bPtmux;\x1b\x1b]52;c;aGVsbG8=\x07\x1b\\"
    );
    assert!(osc52_sequence(&"x".repeat(100 * 1024 + 1), false).is_err());
}

#[test]
fn failed_system_copy_remains_available_across_text_fields() {
    assert!(write_system_clipboard("retained copy").is_err());
    assert_eq!(read_system_clipboard().as_deref(), Some("retained copy"));
}
