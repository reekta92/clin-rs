use anyhow::{Result, bail};

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Num(f64),
    Str(String),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    EqEq,
    BangEq,
    Gt,
    Lt,
    GtEq,
    LtEq,
    Bang,
    AndAnd,
    OrOr,
    LParen,
    RParen,
    Dot,
    Comma,
}

pub fn lex(input: &str) -> Result<Vec<Tok>> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }

        match c {
            '+' => {
                chars.next();
                tokens.push(Tok::Plus);
            }
            '-' => {
                chars.next();
                tokens.push(Tok::Minus);
            }
            '*' => {
                chars.next();
                tokens.push(Tok::Star);
            }
            '/' => {
                chars.next();
                tokens.push(Tok::Slash);
            }
            '%' => {
                chars.next();
                tokens.push(Tok::Percent);
            }
            '(' => {
                chars.next();
                tokens.push(Tok::LParen);
            }
            ')' => {
                chars.next();
                tokens.push(Tok::RParen);
            }
            '.' => {
                // If it is followed by a digit, let's treat it as part of a number,
                // or just let numbers start with a digit. Let's do dot token.
                chars.next();
                tokens.push(Tok::Dot);
            }
            ',' => {
                chars.next();
                tokens.push(Tok::Comma);
            }
            '=' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Tok::EqEq);
                } else {
                    bail!("Unexpected character '=' (expected '==')");
                }
            }
            '!' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Tok::BangEq);
                } else {
                    tokens.push(Tok::Bang);
                }
            }
            '>' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Tok::GtEq);
                } else {
                    tokens.push(Tok::Gt);
                }
            }
            '<' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Tok::LtEq);
                } else {
                    tokens.push(Tok::Lt);
                }
            }
            '&' => {
                chars.next();
                if chars.peek() == Some(&'&') {
                    chars.next();
                    tokens.push(Tok::AndAnd);
                } else {
                    bail!("Unexpected character '&' (expected '&&')");
                }
            }
            '|' => {
                chars.next();
                if chars.peek() == Some(&'|') {
                    chars.next();
                    tokens.push(Tok::OrOr);
                } else {
                    bail!("Unexpected character '|' (expected '||')");
                }
            }
            '"' | '\'' => {
                let quote = c;
                chars.next();
                let mut s = String::new();
                let mut escaped = false;
                loop {
                    match chars.next() {
                        Some(ch) if escaped => {
                            match ch {
                                'n' => s.push('\n'),
                                't' => s.push('\t'),
                                'r' => s.push('\r'),
                                _ => s.push(ch),
                            }
                            escaped = false;
                        }
                        Some('\\') => {
                            escaped = true;
                        }
                        Some(ch) if ch == quote => {
                            break;
                        }
                        Some(ch) => {
                            s.push(ch);
                        }
                        None => bail!("Unterminated string literal"),
                    }
                }
                tokens.push(Tok::Str(s));
            }
            _ if c.is_ascii_digit() => {
                let mut num_str = String::new();
                while let Some(&ch) = chars.peek() {
                    if ch.is_ascii_digit() {
                        num_str.push(ch);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if chars.peek() == Some(&'.') {
                    // Peek ahead to see if the char after dot is a digit
                    let mut temp_chars = chars.clone();
                    temp_chars.next(); // consume dot
                    if temp_chars.peek().is_some_and(|ch| ch.is_ascii_digit()) {
                        num_str.push('.');
                        chars.next(); // consume dot
                        while let Some(&ch) = chars.peek() {
                            if ch.is_ascii_digit() {
                                num_str.push(ch);
                                chars.next();
                            } else {
                                break;
                            }
                        }
                    }
                }
                let val: f64 = num_str.parse()?;
                tokens.push(Tok::Num(val));
            }
            _ if c.is_alphabetic() || c == '_' => {
                let mut ident = String::new();
                while let Some(&ch) = chars.peek() {
                    if ch.is_alphanumeric() || ch == '_' {
                        ident.push(ch);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Tok::Ident(ident));
            }
            _ => {
                bail!("Unexpected character in expression: '{}'", c);
            }
        }
    }

    Ok(tokens)
}
