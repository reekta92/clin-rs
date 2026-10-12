//! ECMAScript RegExp only: typed inputs, restricted intrinsics, bounded time/memory/stack.

use super::eval::arity;
use super::value::{Text, Value};
use anyhow::{Result, anyhow, bail};
use rquickjs::{Context, Ctx, Exception, Function, Object, Runtime, context::intrinsic};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

const TIME_LIMIT: Duration = Duration::from_millis(25);

struct RegexVm {
    context: Context,
    deadline: Rc<Cell<Instant>>,
}
impl RegexVm {
    fn new() -> Result<Self> {
        let runtime = Runtime::new()?;
        runtime.set_memory_limit(32 * 1024 * 1024);
        runtime.set_max_stack_size(512 * 1024);
        let deadline = Rc::new(Cell::new(Instant::now() + TIME_LIMIT));
        let interrupt_deadline = deadline.clone();
        // QuickJS checks this callback within the RegExp backtracking interpreter too.
        runtime.set_interrupt_handler(Some(Box::new(move || {
            Instant::now() >= interrupt_deadline.get()
        })));
        let context = Context::custom::<(
            intrinsic::Eval,
            intrinsic::RegExpCompiler,
            intrinsic::RegExp,
            intrinsic::MapSet,
        )>(&runtime)?;
        context.with(|ctx| ctx.eval::<(), _>(include_str!("regexp.js")))?;
        Ok(Self { context, deadline })
    }
    fn run<T>(
        &self,
        operation: impl for<'js> FnOnce(Ctx<'js>) -> rquickjs::Result<T>,
    ) -> Result<T> {
        self.deadline.set(Instant::now() + TIME_LIMIT);
        self.context.with(|ctx| {
            operation(ctx.clone()).map_err(|error| {
                let caught = ctx.catch();
                let message = caught
                    .into_object()
                    .and_then(Exception::from_object)
                    .and_then(|error| error.message());
                anyhow!(
                    "regular expression failed: {}",
                    message.unwrap_or_else(|| error.to_string())
                )
            })
        })
    }
}
thread_local! { static VM: RefCell<Option<RegexVm>> = const { RefCell::new(None) }; }

fn run<T>(operation: impl for<'js> FnOnce(Ctx<'js>) -> rquickjs::Result<T>) -> Result<T> {
    VM.with(|vm| {
        let mut vm = vm.borrow_mut();
        if vm.is_none() {
            *vm = Some(RegexVm::new()?);
        }
        vm.as_ref().expect("initialized regex VM").run(operation)
    })
}
fn function<'js>(ctx: &Ctx<'js>, name: &str) -> rquickjs::Result<Function<'js>> {
    let adapter: Object = ctx.globals().get("basesRegExp")?;
    adapter.get(name)
}

pub(crate) fn method(pattern: &str, flags: &str, name: &str, values: &[Value]) -> Result<Value> {
    if name != "matches" {
        bail!("unsupported regexp method {name:?}");
    }
    arity(values.len(), 1, 1)?;
    let input = &values[0].as_text()?.0;
    Ok(Value::Boolean(run(|ctx| {
        function(&ctx, "matches")?.call((pattern, flags, input.clone()))
    })?))
}

pub(crate) fn replace(text: &Text, pattern: &Value, replacement: &Text) -> Result<Text> {
    let result: Vec<u16> = match pattern {
        Value::String(pattern) => run(|ctx| {
            function(&ctx, "replace")?.call((
                pattern.0.clone(),
                "",
                false,
                text.0.clone(),
                replacement.0.clone(),
            ))
        })?,
        Value::Regex { pattern, flags } => run(|ctx| {
            function(&ctx, "replace")?.call((
                pattern.as_str(),
                flags.as_str(),
                true,
                text.0.clone(),
                replacement.0.clone(),
            ))
        })?,
        _ => bail!("replace pattern must be string or regexp"),
    };
    Ok(Text(result))
}

pub(crate) fn split(text: &Text, separator: &Value, limit: usize) -> Result<Vec<Value>> {
    let result: Vec<Option<Vec<u16>>> = match separator {
        Value::String(separator) => run(|ctx| {
            function(&ctx, "split")?.call((
                separator.0.clone(),
                "",
                false,
                text.0.clone(),
                limit as u32,
            ))
        })?,
        Value::Regex { pattern, flags } => run(|ctx| {
            function(&ctx, "split")?.call((
                pattern.as_str(),
                flags.as_str(),
                true,
                text.0.clone(),
                limit as u32,
            ))
        })?,
        _ => bail!("split separator must be string or regexp"),
    };
    Ok(result
        .into_iter()
        .map(|value| value.map_or(Value::Null, |value| Value::String(Text(value))))
        .collect())
}
