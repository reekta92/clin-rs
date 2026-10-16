use super::read_note_stdin;
use clap::Parser;

#[test]
fn stdin_preserves_utf8_and_line_endings() {
    let input = "first\r\n世界\n";
    assert_eq!(read_note_stdin(input.as_bytes()).unwrap(), input);
    assert_eq!(read_note_stdin(&b""[..]).unwrap(), "");
}

#[test]
fn stdin_rejects_invalid_utf8_and_bounds_unending_input() {
    assert!(read_note_stdin(&b"bad\xff"[..]).is_err());
    assert!(
        read_note_stdin(std::io::repeat(b'x'))
            .unwrap_err()
            .to_string()
            .contains("16 MiB")
    );
}

#[test]
fn explicit_stdin_and_body_are_mutually_exclusive() {
    assert!(
        crate::cli::Cli::try_parse_from(["clin", "notes", "new", "--stdin", "--body", "text"])
            .is_err()
    );
    assert!(crate::cli::Cli::try_parse_from(["clin", "notes", "new", "--stdin", "Title"]).is_ok());
}
