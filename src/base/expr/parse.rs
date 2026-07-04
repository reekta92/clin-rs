use super::lex::{Tok, lex};
use super::value::Value;
use anyhow::{Context, Result, bail};

#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Lit(Value),
    Path(Vec<String>),
    Neg(Box<Expr>),
    Not(Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
    Method(Box<Expr>, String, Vec<Expr>),
}

pub fn parse(s: &str) -> Result<Expr> {
    let tokens = lex(s).context("lexing failed")?;
    let mut parser = Parser::new(tokens);
    parser.parse_expr(0)
}

struct Parser {
    tokens: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Tok>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Tok> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<Tok> {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    fn expect(&mut self, expected: Tok) -> Result<()> {
        match self.next() {
            Some(tok) if tok == expected => Ok(()),
            Some(tok) => bail!("Expected {:?}, got {:?}", expected, tok),
            None => bail!("Expected {:?}, got EOF", expected),
        }
    }

    fn get_precedence(tok: &Tok) -> u8 {
        match tok {
            Tok::OrOr => 1,
            Tok::AndAnd => 2,
            Tok::EqEq | Tok::BangEq | Tok::Gt | Tok::Lt | Tok::GtEq | Tok::LtEq => 3,
            Tok::Plus | Tok::Minus => 4,
            Tok::Star | Tok::Slash | Tok::Percent => 5,
            _ => 0,
        }
    }

    fn parse_expr(&mut self, min_bp: u8) -> Result<Expr> {
        let mut lhs = self.parse_primary()?;

        while let Some(tok) = self.peek() {
            // Check for postfix dot operator first (binding power is very high)
            if tok == &Tok::Dot {
                self.next(); // consume dot
                let next_tok = self.next().context("Expected identifier after '.'")?;
                if let Tok::Ident(field) = next_tok {
                    if self.peek() == Some(&Tok::LParen) {
                        // Method call: lhs.field(args)
                        let args = self.parse_args()?;
                        lhs = Expr::Method(Box::new(lhs), field, args);
                    } else {
                        // Member access: if lhs is Path, append. Otherwise turn into Method call with 0 args (e.g. .length)
                        match lhs {
                            Expr::Path(mut parts) => {
                                parts.push(field);
                                lhs = Expr::Path(parts);
                            }
                            _ => {
                                lhs = Expr::Method(Box::new(lhs), field, Vec::new());
                            }
                        }
                    }
                    continue;
                } else {
                    bail!("Expected identifier after '.', got {:?}", next_tok);
                }
            }

            let bp = Self::get_precedence(tok);
            if bp == 0 || bp < min_bp {
                break;
            }

            let op_tok = self.next().expect("operator token");
            let op = match op_tok {
                Tok::Plus => BinOp::Add,
                Tok::Minus => BinOp::Sub,
                Tok::Star => BinOp::Mul,
                Tok::Slash => BinOp::Div,
                Tok::Percent => BinOp::Mod,
                Tok::EqEq => BinOp::Eq,
                Tok::BangEq => BinOp::Ne,
                Tok::Gt => BinOp::Gt,
                Tok::Lt => BinOp::Lt,
                Tok::GtEq => BinOp::Ge,
                Tok::LtEq => BinOp::Le,
                Tok::AndAnd => BinOp::And,
                Tok::OrOr => BinOp::Or,
                _ => unreachable!(),
            };

            let rhs = self.parse_expr(bp + 1)?;
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(rhs));
        }

        Ok(lhs)
    }

    fn parse_primary(&mut self) -> Result<Expr> {
        let tok = self.next().context("Unexpected EOF")?;
        match tok {
            Tok::Num(n) => Ok(Expr::Lit(Value::Num(n))),
            Tok::Str(s) => Ok(Expr::Lit(Value::Str(s))),
            Tok::Ident(id) => {
                // Check if boolean or null literal
                if id == "true" {
                    return Ok(Expr::Lit(Value::Bool(true)));
                } else if id == "false" {
                    return Ok(Expr::Lit(Value::Bool(false)));
                } else if id == "null" {
                    return Ok(Expr::Lit(Value::Null));
                }

                if self.peek() == Some(&Tok::LParen) {
                    let args = self.parse_args()?;
                    Ok(Expr::Call(id, args))
                } else {
                    Ok(Expr::Path(vec![id]))
                }
            }
            Tok::LParen => {
                let expr = self.parse_expr(0)?;
                self.expect(Tok::RParen)?;
                Ok(expr)
            }
            Tok::Minus => {
                let expr = self.parse_expr(6)?; // high binding power
                Ok(Expr::Neg(Box::new(expr)))
            }
            Tok::Bang => {
                let expr = self.parse_expr(6)?;
                Ok(Expr::Not(Box::new(expr)))
            }
            _ => bail!("Unexpected token in primary expression: {:?}", tok),
        }
    }

    fn parse_args(&mut self) -> Result<Vec<Expr>> {
        self.expect(Tok::LParen)?;
        let mut args = Vec::new();
        if self.peek() == Some(&Tok::RParen) {
            self.next();
            return Ok(args);
        }

        loop {
            let arg = self.parse_expr(0)?;
            args.push(arg);

            match self.next() {
                Some(Tok::Comma) => continue,
                Some(Tok::RParen) => break,
                Some(tok) => bail!("Expected ',' or ')', got {:?}", tok),
                None => bail!("Expected ',' or ')', got EOF"),
            }
        }

        Ok(args)
    }
}
