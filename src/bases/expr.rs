//! Bounded expression grammar. No assignments, JavaScript execution, or plugin callbacks.

use anyhow::{Context, Result, bail};
use std::collections::BTreeSet;

const MAX_SOURCE: usize = 64 * 1024;
const MAX_TOKENS: usize = 4096;
const MAX_DEPTH: usize = 64;

pub(crate) const GLOBAL_FUNCTIONS: &[&str] = &[
    "escapeHTML",
    "date",
    "duration",
    "file",
    "html",
    "if",
    "image",
    "icon",
    "link",
    "list",
    "max",
    "min",
    "now",
    "number",
    "today",
    "random",
];
pub(crate) const METHODS: &[&str] = &[
    "isTruthy",
    "isType",
    "toString",
    "date",
    "format",
    "time",
    "relative",
    "isEmpty",
    "contains",
    "containsAll",
    "containsAny",
    "endsWith",
    "lower",
    "replace",
    "repeat",
    "reverse",
    "slice",
    "split",
    "startsWith",
    "title",
    "trim",
    "abs",
    "ceil",
    "floor",
    "round",
    "toFixed",
    "filter",
    "flat",
    "join",
    "map",
    "reduce",
    "sort",
    "unique",
    "asFile",
    "linksTo",
    "asLink",
    "hasLink",
    "hasProperty",
    "hasTag",
    "inFolder",
    "keys",
    "values",
    "matches",
    "mean",
];

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Literal {
    Null,
    Boolean(bool),
    Number(f64),
    String(Vec<u16>),
    Regex { pattern: String, flags: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Operator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
    Not,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Kind {
    Literal(Literal),
    Name(String),
    Unary(Operator, Box<Node>),
    Binary(Operator, Box<Node>, Box<Node>),
    Member(Box<Node>, Box<Node>),
    Call(Box<Node>, Vec<Node>),
    List(Vec<Node>),
    Object(Vec<(Vec<u16>, Node)>),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Node {
    pub kind: Kind,
    pub offset: usize,
}

impl Node {
    fn new(kind: Kind, offset: usize) -> Self {
        Self { kind, offset }
    }

    pub(crate) fn string(&self) -> Option<String> {
        match &self.kind {
            Kind::Literal(Literal::String(value)) => String::from_utf16(value).ok(),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Expression {
    pub(crate) root: Node,
    dependencies: BTreeSet<String>,
}

impl Expression {
    pub fn parse(source: &str) -> Result<Self> {
        if source.len() > MAX_SOURCE {
            bail!("expression exceeds 64 KiB");
        }
        let mut lexer = Lexer {
            source,
            offset: 0,
            tokens: 0,
        };
        let current = lexer.next()?;
        let mut parser = Parser {
            lexer,
            current,
            depth: 0,
        };
        let root = parser.expression(0)?;
        if parser.current.kind != TokenKind::End {
            bail!("unexpected token at byte {}", parser.current.offset);
        }
        let mut expression = Self {
            root,
            dependencies: BTreeSet::new(),
        };
        let mut pending = vec![(&expression.root, 1)];
        while let Some((node, depth)) = pending.pop() {
            if depth > MAX_DEPTH {
                bail!("expression nesting exceeds 64 levels");
            }
            match &node.kind {
                Kind::Member(receiver, key) => {
                    if matches!(&receiver.kind, Kind::Name(name) if name == "formula")
                        && let Some(name) = key.string()
                    {
                        expression.dependencies.insert(name);
                    }
                    pending.extend([(receiver.as_ref(), depth + 1), (key.as_ref(), depth + 1)]);
                }
                Kind::Call(target, arguments) => {
                    match &target.kind {
                        Kind::Name(name) if GLOBAL_FUNCTIONS.contains(&name.as_str()) => {}
                        Kind::Member(_, key)
                            if key
                                .string()
                                .is_some_and(|name| METHODS.contains(&name.as_str())) => {}
                        _ => bail!("unsupported Bases function at byte {}", target.offset),
                    }
                    pending.push((target, depth + 1));
                    pending.extend(arguments.iter().map(|node| (node, depth + 1)));
                }
                Kind::Unary(_, child) => pending.push((child, depth + 1)),
                Kind::Binary(_, left, right) => {
                    pending.extend([(left.as_ref(), depth + 1), (right.as_ref(), depth + 1)])
                }
                Kind::List(nodes) => pending.extend(nodes.iter().map(|node| (node, depth + 1))),
                Kind::Object(entries) => {
                    pending.extend(entries.iter().map(|(_, node)| (node, depth + 1)))
                }
                Kind::Literal(_) | Kind::Name(_) => {}
            }
        }
        Ok(expression)
    }

    pub fn dependencies(&self) -> &BTreeSet<String> {
        &self.dependencies
    }
}

#[derive(Clone, Debug, PartialEq)]
enum TokenKind {
    Literal(Literal),
    Name(String),
    Operator(Operator),
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    LeftBrace,
    RightBrace,
    Dot,
    Comma,
    Colon,
    End,
}

struct Token {
    kind: TokenKind,
    offset: usize,
}
struct Lexer<'a> {
    source: &'a str,
    offset: usize,
    tokens: usize,
}

impl Lexer<'_> {
    fn next(&mut self) -> Result<Token> {
        self.tokens += 1;
        if self.tokens > MAX_TOKENS {
            bail!("expression exceeds 4096 tokens");
        }
        while self.peek().is_some_and(char::is_whitespace) {
            self.take();
        }
        let offset = self.offset;
        let Some(ch) = self.take() else {
            return Ok(Token {
                kind: TokenKind::End,
                offset,
            });
        };
        let kind = match ch {
            '(' => TokenKind::LeftParen,
            ')' => TokenKind::RightParen,
            '[' => TokenKind::LeftBracket,
            ']' => TokenKind::RightBracket,
            '{' => TokenKind::LeftBrace,
            '}' => TokenKind::RightBrace,
            ',' => TokenKind::Comma,
            ':' => TokenKind::Colon,
            '+' => TokenKind::Operator(Operator::Add),
            '-' => TokenKind::Operator(Operator::Subtract),
            '*' => TokenKind::Operator(Operator::Multiply),
            '/' => TokenKind::Operator(Operator::Divide),
            '%' => TokenKind::Operator(Operator::Modulo),
            '!' => TokenKind::Operator(if self.eat('=') {
                Operator::NotEqual
            } else {
                Operator::Not
            }),
            '=' if self.eat('=') => TokenKind::Operator(Operator::Equal),
            '<' => TokenKind::Operator(if self.eat('=') {
                Operator::LessEqual
            } else {
                Operator::Less
            }),
            '>' => TokenKind::Operator(if self.eat('=') {
                Operator::GreaterEqual
            } else {
                Operator::Greater
            }),
            '&' if self.eat('&') => TokenKind::Operator(Operator::And),
            '|' if self.eat('|') => TokenKind::Operator(Operator::Or),
            '\'' | '"' => TokenKind::Literal(Literal::String(self.quoted(ch)?)),
            '0'..='9' => TokenKind::Literal(Literal::Number(self.number(offset)?)),
            '.' if self.peek().is_some_and(|ch| ch.is_ascii_digit()) => {
                TokenKind::Literal(Literal::Number(self.number(offset)?))
            }
            '.' => TokenKind::Dot,
            ch if ch.is_alphabetic() || ch == '_' || ch == '$' => {
                while self
                    .peek()
                    .is_some_and(|ch| ch.is_alphanumeric() || ch == '_' || ch == '$')
                {
                    self.take();
                }
                match &self.source[offset..self.offset] {
                    "null" => TokenKind::Literal(Literal::Null),
                    "true" => TokenKind::Literal(Literal::Boolean(true)),
                    "false" => TokenKind::Literal(Literal::Boolean(false)),
                    name => TokenKind::Name(name.to_owned()),
                }
            }
            _ => bail!("unexpected character {ch:?} at byte {offset}"),
        };
        Ok(Token { kind, offset })
    }

    fn peek(&self) -> Option<char> {
        self.source[self.offset..].chars().next()
    }
    fn take(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.offset += ch.len_utf8();
        Some(ch)
    }
    fn eat(&mut self, ch: char) -> bool {
        if self.peek() == Some(ch) {
            self.take();
            true
        } else {
            false
        }
    }

    fn number(&mut self, start: usize) -> Result<f64> {
        if self.source[start..].starts_with('0') {
            let radix = match self.peek() {
                Some('x' | 'X') => Some(16),
                Some('b' | 'B') => Some(2),
                Some('o' | 'O') => Some(8),
                _ => None,
            };
            if let Some(radix) = radix {
                self.take();
                let digits = self.offset;
                let mut value = 0.0;
                while let Some(digit) = self.peek().and_then(|ch| ch.to_digit(radix)) {
                    self.take();
                    value = value * f64::from(radix) + f64::from(digit);
                }
                if digits == self.offset {
                    bail!("missing numeric digits at byte {start}");
                }
                return Ok(value);
            }
        }
        while self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
            self.take();
        }
        if self.peek() == Some('.')
            && self.source[self.offset + 1..]
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_digit())
        {
            self.take();
            while self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
                self.take();
            }
        }
        if matches!(self.peek(), Some('e' | 'E')) {
            self.take();
            if matches!(self.peek(), Some('+' | '-')) {
                self.take();
            }
            let digits = self.offset;
            while self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
                self.take();
            }
            if digits == self.offset {
                bail!("missing exponent digits at byte {start}");
            }
        }
        self.source[start..self.offset]
            .parse()
            .context("invalid number")
    }

    fn quoted(&mut self, quote: char) -> Result<Vec<u16>> {
        let mut value = Vec::new();
        while let Some(ch) = self.take() {
            if ch == quote {
                return Ok(value);
            }
            if ch == '\n' || ch == '\r' {
                bail!("unescaped line break in string");
            }
            let ch = if ch == '\\' {
                match self.take().context("unterminated string escape")? {
                    'u' if self.eat('{') => {
                        let start = self.offset;
                        while self.peek().is_some_and(|ch| ch.is_ascii_hexdigit()) {
                            self.take();
                        }
                        let scalar = u32::from_str_radix(&self.source[start..self.offset], 16)
                            .context("invalid Unicode escape")?;
                        if !self.eat('}') {
                            bail!("missing Unicode escape closing brace");
                        }
                        char::from_u32(scalar).context("invalid Unicode scalar")?
                    }
                    'u' => {
                        value.push(self.hex(4)?);
                        continue;
                    }
                    'x' => {
                        value.push(self.hex(2)?);
                        continue;
                    }
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    'b' => '\u{8}',
                    'f' => '\u{c}',
                    'v' => '\u{b}',
                    '0' => '\0',
                    '\n' => continue,
                    '\r' => {
                        self.eat('\n');
                        continue;
                    }
                    escaped => escaped,
                }
            } else {
                ch
            };
            value.extend(ch.encode_utf16(&mut [0; 2]).iter().copied());
        }
        bail!("unterminated string")
    }

    fn hex(&mut self, count: usize) -> Result<u16> {
        let mut value = 0;
        for _ in 0..count {
            let digit = self
                .take()
                .and_then(|ch| ch.to_digit(16))
                .context("invalid hex string escape")?;
            value = value * 16 + digit as u16;
        }
        Ok(value)
    }

    fn regex(&mut self) -> Result<Literal> {
        let start = self.offset;
        let mut escaped = false;
        let mut in_class = false;
        while let Some(ch) = self.take() {
            if ch == '\n' || ch == '\r' {
                bail!("line break in regular expression");
            }
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '[' => in_class = true,
                ']' => in_class = false,
                '/' if !in_class => {
                    let pattern = self.source[start..self.offset - 1].to_owned();
                    let start = self.offset;
                    while self.peek().is_some_and(|ch| ch.is_ascii_alphabetic()) {
                        self.take();
                    }
                    let flags = &self.source[start..self.offset];
                    let mut seen = BTreeSet::new();
                    for flag in flags.chars() {
                        if !"dgimsuvy".contains(flag) || !seen.insert(flag) {
                            bail!("invalid regular expression flag {flag:?}");
                        }
                    }
                    if seen.contains(&'u') && seen.contains(&'v') {
                        bail!("regular expression flags u and v are mutually exclusive");
                    }
                    return Ok(Literal::Regex {
                        pattern,
                        flags: flags.to_owned(),
                    });
                }
                _ => {}
            }
        }
        bail!("unterminated regular expression")
    }
}

fn property_key(token: TokenKind) -> Result<String> {
    match token {
        TokenKind::Name(name) => Ok(name),
        TokenKind::Literal(Literal::Boolean(true)) => Ok("true".into()),
        TokenKind::Literal(Literal::Boolean(false)) => Ok("false".into()),
        TokenKind::Literal(Literal::Null) => Ok("null".into()),
        _ => bail!("expected property name"),
    }
}

struct Parser<'a> {
    lexer: Lexer<'a>,
    current: Token,
    depth: usize,
}
impl Parser<'_> {
    fn advance(&mut self) -> Result<Token> {
        let next = self.lexer.next()?;
        Ok(std::mem::replace(&mut self.current, next))
    }
    fn require(&mut self, kind: TokenKind) -> Result<()> {
        if self.current.kind != kind {
            bail!("expected {kind:?} at byte {}", self.current.offset);
        }
        self.advance()?;
        Ok(())
    }

    fn expression(&mut self, minimum: u8) -> Result<Node> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            bail!("expression nesting exceeds 64 levels");
        }
        let mut left = self.prefix()?;
        loop {
            match self.current.kind {
                TokenKind::Dot => {
                    let offset = self.advance()?.offset;
                    let name = property_key(self.advance()?.kind)?;
                    left = Node::new(
                        Kind::Member(
                            Box::new(left),
                            Box::new(Node::new(
                                Kind::Literal(Literal::String(name.encode_utf16().collect())),
                                offset,
                            )),
                        ),
                        offset,
                    );
                    continue;
                }
                TokenKind::LeftBracket => {
                    let offset = self.advance()?.offset;
                    let key = self.expression(0)?;
                    self.require(TokenKind::RightBracket)?;
                    left = Node::new(Kind::Member(Box::new(left), Box::new(key)), offset);
                    continue;
                }
                TokenKind::LeftParen => {
                    let offset = self.advance()?.offset;
                    let arguments = self.arguments(TokenKind::RightParen)?;
                    left = Node::new(Kind::Call(Box::new(left), arguments), offset);
                    continue;
                }
                _ => {}
            }
            let TokenKind::Operator(operator) = self.current.kind else {
                break;
            };
            let priority = match operator {
                Operator::Or => 1,
                Operator::And => 2,
                Operator::Equal | Operator::NotEqual => 3,
                Operator::Less
                | Operator::LessEqual
                | Operator::Greater
                | Operator::GreaterEqual => 4,
                Operator::Add | Operator::Subtract => 5,
                Operator::Multiply | Operator::Divide | Operator::Modulo => 6,
                Operator::Not => break,
            };
            if priority < minimum {
                break;
            }
            let offset = self.advance()?.offset;
            let right = self.expression(priority + 1)?;
            left = Node::new(
                Kind::Binary(operator, Box::new(left), Box::new(right)),
                offset,
            );
        }
        self.depth -= 1;
        Ok(left)
    }

    fn prefix(&mut self) -> Result<Node> {
        if self.current.kind == TokenKind::Operator(Operator::Divide) {
            let offset = self.current.offset;
            let regex = self.lexer.regex()?;
            self.current = self.lexer.next()?;
            return Ok(Node::new(Kind::Literal(regex), offset));
        }
        let token = self.advance()?;
        let kind = match token.kind {
            TokenKind::Literal(value) => Kind::Literal(value),
            TokenKind::Name(name) => Kind::Name(name),
            TokenKind::Operator(
                operator @ (Operator::Not | Operator::Add | Operator::Subtract),
            ) => Kind::Unary(operator, Box::new(self.expression(7)?)),
            TokenKind::LeftParen => {
                let expression = self.expression(0)?;
                self.require(TokenKind::RightParen)?;
                return Ok(expression);
            }
            TokenKind::LeftBracket => Kind::List(self.arguments(TokenKind::RightBracket)?),
            TokenKind::LeftBrace => {
                let mut entries = Vec::new();
                while self.current.kind != TokenKind::RightBrace {
                    let key = match self.advance()?.kind {
                        TokenKind::Literal(Literal::String(key)) => key,
                        token => property_key(token)?.encode_utf16().collect(),
                    };
                    self.require(TokenKind::Colon)?;
                    entries.push((key, self.expression(0)?));
                    if self.current.kind != TokenKind::Comma {
                        break;
                    }
                    self.advance()?;
                }
                self.require(TokenKind::RightBrace)?;
                Kind::Object(entries)
            }
            _ => bail!("expected expression at byte {}", token.offset),
        };
        Ok(Node::new(kind, token.offset))
    }

    fn arguments(&mut self, closing: TokenKind) -> Result<Vec<Node>> {
        let mut arguments = Vec::new();
        while self.current.kind != closing {
            arguments.push(self.expression(0)?);
            if self.current.kind != TokenKind::Comma {
                break;
            }
            self.advance()?;
        }
        self.require(closing)?;
        Ok(arguments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_precedence_and_left_associativity() {
        let expression = Expression::parse("10 - 3 - 2 * 2 == 3 && !false || true").unwrap();
        let Kind::Binary(Operator::Or, left, _) = &expression.root.kind else {
            panic!()
        };
        let Kind::Binary(Operator::And, left, _) = &left.kind else {
            panic!()
        };
        let Kind::Binary(Operator::Equal, left, _) = &left.kind else {
            panic!()
        };
        let Kind::Binary(Operator::Subtract, left, right) = &left.kind else {
            panic!()
        };
        assert!(matches!(left.kind, Kind::Binary(Operator::Subtract, ..)));
        assert!(matches!(right.kind, Kind::Binary(Operator::Multiply, ..)));
    }
    #[test]
    fn supports_namespaces_brackets_objects_and_implicit_iteration() {
        for source in [
            "note['with spaces']",
            "note.true",
            "{true: 1, null: 2}",
            "note.list[0].value",
            "[1,2,3].map(value + index).filter(value > 2)",
            "{'a': 1, b: 2}.keys()",
            "if(done, file.asLink(), null)",
            "formula['total price'] + formula.other",
            "date('2025-01-01') + '1M'",
        ] {
            Expression::parse(source).unwrap();
        }
        let expression =
            Expression::parse("formula['total price'] + formula.other + formula[note.key]")
                .unwrap();
        assert_eq!(
            expression.dependencies,
            BTreeSet::from(["total price".into(), "other".into()])
        );
    }
    #[test]
    fn strings_preserve_javascript_utf16_units() {
        let expression = Expression::parse(r#"'😀\uD800\u{1F600}\x41\n'"#).unwrap();
        assert_eq!(
            expression.root.kind,
            Kind::Literal(Literal::String(vec![
                0xd83d, 0xde00, 0xd800, 0xd83d, 0xde00, 65, 10
            ]))
        );
        assert!(Expression::parse(r#"'\u{110000}'"#).is_err());
    }
    #[test]
    fn distinguishes_division_from_regex_and_handles_classes_and_escapes() {
        for source in [
            r#"file.size / 2 / 3"#,
            r#"/[/]/.matches('a/b')"#,
            r#"/a\/b/gi.matches('A/B')"#,
            r#"'a'.replace(/(a)/, '$1')"#,
        ] {
            Expression::parse(source).unwrap();
        }
        for source in ["/abc", "/abc/gg", "/abc/z", "/abc/uv", "/abc\n/"] {
            assert!(Expression::parse(source).is_err(), "{source}");
        }
    }
    #[test]
    fn numeric_member_access_exponents_and_radices() {
        for source in [
            "123.toString()",
            "(2.5).round()",
            ".5 + 1e-3",
            "0xff + 0b10 + 0o12",
        ] {
            Expression::parse(source).unwrap();
        }
        for source in ["1e", "0x", "12junk"] {
            assert!(Expression::parse(source).is_err());
        }
    }
    #[test]
    fn rejects_scripts_unknown_functions_and_bad_syntax() {
        for source in [
            "a = 1",
            "a; file('secret')",
            "()=>1",
            "new Date()",
            "eval('1')",
            "file.constructor('x')",
            "[1,,2]",
            "[1",
            "a ? 1 : 2",
            "file['x' + 'y']()",
        ] {
            assert!(Expression::parse(source).is_err(), "{source}");
        }
    }
    #[test]
    fn bounds_parentheses_left_chains_tokens_and_source() {
        for source in [
            format!("{}1{}", "(".repeat(100), ")".repeat(100)),
            "1+".repeat(200) + "1",
            "a".repeat(MAX_SOURCE + 1),
            format!("[{}0]", "0,".repeat(MAX_TOKENS)),
        ] {
            assert!(Expression::parse(&source).is_err());
        }
    }
}
