pub mod eval;
pub mod funcs;
pub mod lex;
pub mod parse;
pub mod value;

pub use eval::{EvalCtx, eval};
pub use parse::{BinOp, Expr, parse};
pub use value::Value;

use crate::base::model::{FilterNode, FilterNodeMap};
use anyhow::Result;
use std::collections::HashMap;

#[allow(clippy::implicit_hasher)]
pub fn eval_filter(
    node: &FilterNode,
    ctx: &mut EvalCtx<'_>,
    cache: &mut HashMap<String, Expr>,
) -> Result<bool> {
    match node {
        FilterNode::Expr(s) => {
            let e = cache
                .entry(s.clone())
                .or_insert_with(|| parse(s).unwrap_or(Expr::Lit(Value::Null)));
            Ok(eval(e, ctx).map(|v| v.is_truthy()).unwrap_or(false))
        }
        FilterNode::Map(map) => eval_filter_map(map, ctx, cache),
    }
}

#[allow(clippy::implicit_hasher)]
fn eval_filter_map(
    map: &FilterNodeMap,
    ctx: &mut EvalCtx<'_>,
    cache: &mut HashMap<String, Expr>,
) -> Result<bool> {
    if let Some(and_nodes) = &map.and {
        for n in and_nodes {
            if !eval_filter(n, ctx, cache)? {
                return Ok(false);
            }
        }
        return Ok(true);
    }
    if let Some(or_nodes) = &map.or {
        for n in or_nodes {
            if eval_filter(n, ctx, cache)? {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    if let Some(not_nodes) = &map.not {
        // "none of the following"
        for n in not_nodes {
            if eval_filter(n, ctx, cache)? {
                return Ok(false);
            }
        }
        return Ok(true);
    }
    if let Some(expr_str) = &map.expr {
        let e = cache
            .entry(expr_str.clone())
            .or_insert_with(|| parse(expr_str).unwrap_or(Expr::Lit(Value::Null)));
        return Ok(eval(e, ctx).map(|v| v.is_truthy()).unwrap_or(false));
    }
    Ok(true)
}
