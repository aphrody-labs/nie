//! Host function registry: the natives that the game binary registers into the Lua state.
//!
//! Until a host function has real semantics it is a recording stub: it appends one trace line
//! per call and returns nothing. The trace is what differential tests compare with the oracle.

use super::state::{Args, Ctx, LResult};
use super::lstr;
use super::value::{Value, tag};

/// Renders a value for traces; identical to the oracle's rendering.
#[must_use]
pub fn render(v: Value) -> String {
    match v.tag {
        tag::NIL => "nil".into(),
        tag::BOOL => (v.bits != 0).to_string(),
        tag::NUM => String::from_utf8_lossy(&lstr::number_to_string(v.as_num())).into_owned(),
        // SAFETY: STR tag.
        tag::STR => format!("{:?}", String::from_utf8_lossy(unsafe { v.str_bytes() })),
        tag::TABLE => "table".into(),
        tag::FUNC => "function".into(),
        tag::THREAD => "thread".into(),
        _ => "userdata".into(),
    }
}

fn stub(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let name = a.data();
    // SAFETY: the bound data is the interned name.
    let mut line = String::from_utf8_lossy(unsafe { name.str_bytes() }).into_owned();
    line.push('(');
    for i in 0..a.n {
        if i > 0 {
            line.push_str(", ");
        }
        line.push_str(&render(a.get(i)));
    }
    line.push(')');
    ctx.trace.push(line);
    a.none()
}

/// Registers `names` as recording stubs in the global table.
pub fn install_recording_hosts<'a>(ctx: &mut Ctx, names: impl IntoIterator<Item = &'a str>) {
    for n in names {
        let data = ctx.str(n.as_bytes());
        let f = ctx.new_rust_closure(stub, data);
        ctx.set_global(n, f);
    }
}
