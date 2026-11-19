//! Compile lifted functions into a Cranelift module (object file or JIT) and the call table.

use std::collections::{BTreeMap, HashSet};
use std::str::FromStr;

use anyhow::{Result, anyhow};
use cranelift_codegen::Context;
use cranelift_codegen::isa::{self, OwnedTargetIsa};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_module::{DataDescription, FuncId, Linkage, Module, default_libcall_names};
use cranelift_object::{ObjectBuilder, ObjectModule};

use crate::image::Image;
use crate::lift::{FuncMeta, Lifter, Plan, Root, Why, lifted_sig, symbol};

/// Result of compiling a set of roots.
#[derive(Default)]
pub struct Unit {
    /// Compiled functions with their facts, in compile order.
    pub lifted: Vec<(u64, FuncId, FuncMeta)>,
    /// Roots that were not lifted, with the reason.
    pub failed: Vec<(u64, Why)>,
    /// Stand-ins defined for callees that were not lifted.
    pub stubs: usize,
    pub code_bytes: u64,
}

impl Unit {
    /// Failure counts grouped by reason class (text before the second `:`), most frequent first.
    pub fn failure_classes(&self) -> Vec<(String, usize)> {
        let mut m: BTreeMap<String, usize> = BTreeMap::new();
        for (_, w) in &self.failed {
            let class = w.0.splitn(3, ':').take(2).collect::<Vec<_>>().join(":");
            *m.entry(class).or_default() += 1;
        }
        let mut v: Vec<_> = m.into_iter().collect();
        v.sort_by_key(|(c, n)| (std::cmp::Reverse(*n), c.clone()));
        v
    }
}

/// x86-64 Linux ISA, speed-optimised, position independent.
pub fn host_isa(pic: bool, verifier: bool) -> Result<OwnedTargetIsa> {
    let mut f = settings::builder();
    f.set("opt_level", "speed")?;
    f.set("is_pic", if pic { "true" } else { "false" })?;
    f.set("enable_verifier", if verifier { "true" } else { "false" })?;
    let triple = target_lexicon::Triple::from_str("x86_64-unknown-linux-gnu").map_err(|e| anyhow!("{e}"))?;
    Ok(isa::lookup(triple)?.finish(settings::Flags::new(f))?)
}

pub fn object_module(isa: OwnedTargetIsa, name: &str) -> Result<ObjectModule> {
    Ok(ObjectModule::new(ObjectBuilder::new(isa, name, default_libcall_names())?))
}

/// Lift and define every root, then define trap stubs for undefined callees.
pub fn compile<M: Module>(m: &mut M, img: &Image, plan: &Plan, roots: &[&Root]) -> Result<Unit> {
    let mut lifter = Lifter::new(m.target_config());
    let mut ctx = Context::new();
    let mut unit = Unit::default();
    let mut defined: HashSet<FuncId> = HashSet::new();
    for root in roots {
        match lifter.lift(m, img, plan, root) {
            Ok((id, func, meta)) => {
                ctx.func = func;
                match m.define_function(id, &mut ctx) {
                    Ok(()) => {
                        unit.code_bytes += u64::from(ctx.compiled_code().map_or(0, |c| c.code_info().total_size));
                        defined.insert(id);
                        unit.lifted.push((root.va, id, meta));
                    }
                    Err(e) => unit.failed.push((root.va, Why(format!("compile:{e}")))),
                }
                m.clear_context(&mut ctx);
            }
            Err(w) => unit.failed.push((root.va, w)),
        }
    }
    let undefined: Vec<(FuncId, u64)> = m
        .declarations()
        .get_functions()
        .filter(|(id, d)| !defined.contains(id) && d.linkage == Linkage::Export)
        .filter_map(|(id, d)| {
            let va = u64::from_str_radix(d.name.as_deref()?.strip_prefix("f_")?, 16).ok()?;
            Some((id, va))
        })
        .collect();
    for (id, va) in undefined {
        let (sid, func) = lifter.trap_stub(m, va).map_err(|w| anyhow!("{}", w.0))?;
        debug_assert_eq!(sid, id);
        ctx.func = func;
        m.define_function(sid, &mut ctx)?;
        m.clear_context(&mut ctx);
        unit.stubs += 1;
    }
    Ok(unit)
}

/// `nie_recomp_table`: `[guest va, function address]` pairs, and `nie_recomp_count`.
pub fn define_table<M: Module>(m: &mut M, unit: &Unit) -> Result<()> {
    let n = unit.lifted.len();
    let mut init = vec![0u8; n * 16];
    let mut desc = DataDescription::new();
    for (k, (va, _, _)) in unit.lifted.iter().enumerate() {
        init[k * 16..k * 16 + 8].copy_from_slice(&va.to_le_bytes());
    }
    desc.define(init.into_boxed_slice());
    desc.set_align(16);
    for (k, (_, id, _)) in unit.lifted.iter().enumerate() {
        let fr = m.declare_func_in_data(*id, &mut desc);
        desc.write_function_addr((k * 16 + 8) as u32, fr);
    }
    let tid = m.declare_data("nie_recomp_table", Linkage::Export, true, false)?;
    m.define_data(tid, &desc)?;
    let mut cnt = DataDescription::new();
    cnt.define((n as u64).to_le_bytes().to_vec().into_boxed_slice());
    let cid = m.declare_data("nie_recomp_count", Linkage::Export, false, false)?;
    m.define_data(cid, &cnt)?;
    Ok(())
}

/// Declared signature check helper for tests and tools.
pub fn sym(va: u64) -> String {
    symbol(va)
}

pub fn sig() -> cranelift_codegen::ir::Signature {
    lifted_sig()
}
