//! Ahead-of-time compiler from Lua 5.2 bytecode to native code.
//!
//! Pipeline: [`crate::bytecode`] (parse) -> [`ir`] (typed register IR) -> [`clif`] (Cranelift,
//! JIT module or object file) or [`rustgen`] (reviewable Rust source). The generated code runs
//! on [`crate::rt`]; there is no interpreter in the path.

pub mod clif;
pub mod ir;
pub mod rustgen;

use cranelift_codegen::settings::{self, Configurable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{DataDescription, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};

use crate::bytecode::{self, Chunk, Constant, Prototype};
use crate::rt::heap::ProtoRt;
use crate::rt::state::{Ctx, NativeFn};
use crate::rt::value::{Value, tag};

/// Compilation failure.
#[derive(Debug, thiserror::Error)]
pub enum AotError {
    /// Malformed bytecode.
    #[error(transparent)]
    Lower(#[from] ir::LowerError),
    /// Cranelift module error.
    #[error("cranelift: {0}")]
    Module(String),
    /// Target configuration error.
    #[error("target: {0}")]
    Target(String),
}

impl From<cranelift_module::ModuleError> for AotError {
    fn from(e: cranelift_module::ModuleError) -> Self {
        Self::Module(e.to_string())
    }
}

fn isa() -> Result<std::sync::Arc<dyn cranelift_codegen::isa::TargetIsa>, AotError> {
    let mut flags = settings::builder();
    for (k, v) in [("opt_level", "speed"), ("is_pic", "true")] {
        flags
            .set(k, v)
            .map_err(|e| AotError::Target(e.to_string()))?;
    }
    let builder = cranelift_native::builder().map_err(|e| AotError::Target(e.to_string()))?;
    builder
        .finish(settings::Flags::new(flags))
        .map_err(|e| AotError::Target(e.to_string()))
}

fn collect<'a>(p: &'a Prototype, out: &mut Vec<&'a Prototype>) {
    out.push(p);
    for c in &p.protos {
        collect(c, out);
    }
}

/// A chunk compiled into executable memory of this process.
pub struct JitChunk {
    // Kept alive: owns the executable memory behind `funcs`.
    _module: JITModule,
    funcs: Vec<NativeFn>,
    sites: Vec<Vec<ir::Site>>,
    chunk: Chunk,
}

/// Compiles every prototype of `chunk` into an in-process JIT module.
///
/// # Errors
/// [`AotError`] on malformed bytecode or a backend failure.
pub fn compile_jit(chunk: &Chunk) -> Result<JitChunk, AotError> {
    let builder = JITBuilder::with_isa(isa()?, cranelift_module::default_libcall_names());
    let mut module = JITModule::new(builder);
    let mut protos = Vec::new();
    collect(&chunk.main, &mut protos);
    let mut ids = Vec::new();
    for (i, p) in protos.iter().enumerate() {
        let f = ir::lower(p)?;
        ids.push(clif::compile_func(&mut module, &f, &format!("nie_lua_f{i}"), Linkage::Export)?);
    }
    module.finalize_definitions().map_err(|e| AotError::Module(e.to_string()))?;
    let funcs = ids
        .iter()
        .map(|id| {
            let ptr = module.get_finalized_function(*id);
            // SAFETY: the generated function has the `NativeFn` signature.
            unsafe { std::mem::transmute::<*const u8, NativeFn>(ptr) }
        })
        .collect();
    Ok(JitChunk {
        _module: module,
        funcs,
        sites: protos.iter().map(|p| ir::sites(p)).collect(),
        chunk: chunk.clone(),
    })
}

impl JitChunk {
    /// Number of compiled prototypes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.funcs.len()
    }

    /// `true` when nothing was compiled.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.funcs.is_empty()
    }

    /// Instantiates the chunk in `ctx` and returns its main function value.
    pub fn load(&self, ctx: &mut Ctx) -> Value {
        load_image(ctx, &self.chunk, &self.funcs, &self.sites)
    }
}

fn constant_value(ctx: &mut Ctx, c: &Constant) -> Value {
    match c {
        Constant::Nil => Value::NIL,
        Constant::Boolean(b) => Value::bool(*b),
        Constant::Number(n) => Value::num(*n),
        Constant::String(s) => ctx.str(s),
    }
}

fn build_proto(
    ctx: &mut Ctx,
    p: &Prototype,
    funcs: &[NativeFn],
    sites: &[Vec<ir::Site>],
    next: &mut usize,
) -> *const ProtoRt {
    let idx = *next;
    *next += 1;
    let children = p.protos.iter().map(|c| build_proto(ctx, c, funcs, sites, next)).collect();
    let consts: Box<[Value]> = p.constants.iter().map(|c| constant_value(ctx, c)).collect();
    let mut rt = Box::new(ProtoRt {
        f: funcs[idx],
        consts,
        children,
        upvals: p.upvalues.iter().map(|u| (u.in_stack, u.index)).collect(),
        sites: sites
            .get(idx)
            .map(|v| v.iter().map(|s| (s.pc, s.slots.clone())).collect())
            .unwrap_or_default(),
    });
    let ptr: *const ProtoRt = &mut *rt;
    ctx.heap.protos.push(rt);
    ptr
}

/// Binds compiled functions (pre-order, one per prototype) to the constants and upvalue
/// descriptors of `chunk`, and returns the main closure with `_ENV` set to the globals.
///
/// `sites` holds the error descriptions per prototype (see [`ir::sites`]), possibly empty.
///
/// `chunk` may carry empty code vectors: only constants, nesting and upvalue descriptors are read.
pub fn load_image(ctx: &mut Ctx, chunk: &Chunk, funcs: &[NativeFn], sites: &[Vec<ir::Site>]) -> Value {
    let mut next = 0;
    let main = build_proto(ctx, &chunk.main, funcs, sites, &mut next);
    ctx.main_closure(main)
}

fn strip_code(p: &mut Prototype) {
    p.code.clear();
    p.line_info.clear();
    for c in &mut p.protos {
        strip_code(c);
    }
}

/// An ahead-of-time artifact: a relocatable object plus the data needed to bind it.
pub struct ObjectArtifact {
    /// Relocatable object (ELF, Mach-O or COFF per host) exporting `nie_lua_funcs`,
    /// `nie_lua_nfuncs`, `nie_lua_desc` and `nie_lua_desc_len`.
    pub object: Vec<u8>,
    /// Number of functions.
    pub functions: usize,
}

/// Compiles `chunk` to a relocatable object file that links against nothing.
///
/// The object exports a table of function pointers (`nie_lua_funcs`) in prototype pre-order and a
/// descriptor blob (`nie_lua_desc`): the chunk without instructions, from which the loader
/// rebuilds constants, upvalue descriptors and nesting.
///
/// # Errors
/// [`AotError`] on malformed bytecode or a backend failure.
pub fn compile_object(chunk: &Chunk, name: &str) -> Result<ObjectArtifact, AotError> {
    let builder = ObjectBuilder::new(isa()?, name, cranelift_module::default_libcall_names())
        .map_err(|e| AotError::Module(e.to_string()))?;
    let mut module = ObjectModule::new(builder);
    let mut protos = Vec::new();
    collect(&chunk.main, &mut protos);
    let mut ids = Vec::new();
    for (i, p) in protos.iter().enumerate() {
        let f = ir::lower(p)?;
        ids.push(clif::compile_func(&mut module, &f, &format!("nie_lua_f{i}"), Linkage::Local)?);
    }
    // Function pointer table.
    // Writable: the table holds code addresses that a shared object relocates at load time.
    let table = module.declare_data("nie_lua_funcs", Linkage::Export, true, false)?;
    let mut dd = DataDescription::new();
    dd.define_zeroinit(8 * ids.len());
    for (i, id) in ids.iter().enumerate() {
        let fr = module.declare_func_in_data(*id, &mut dd);
        dd.write_function_addr((8 * i) as u32, fr);
    }
    module.define_data(table, &dd)?;
    let mut put = |sym: &str, bytes: Vec<u8>| -> Result<(), AotError> {
        let id = module.declare_data(sym, Linkage::Export, false, false)?;
        let mut d = DataDescription::new();
        d.define(bytes.into_boxed_slice());
        module.define_data(id, &d)?;
        Ok(())
    };
    put("nie_lua_nfuncs", (ids.len() as u64).to_le_bytes().to_vec())?;
    let mut stripped = chunk.clone();
    strip_code(&mut stripped.main);
    let desc = bytecode::encode(&stripped);
    put("nie_lua_desc_len", (desc.len() as u64).to_le_bytes().to_vec())?;
    put("nie_lua_desc", desc)?;
    let site_blob = encode_sites(&protos.iter().map(|p| ir::sites(p)).collect::<Vec<_>>());
    put("nie_lua_sites_len", (site_blob.len() as u64).to_le_bytes().to_vec())?;
    put("nie_lua_sites", site_blob)?;
    let object = module.finish().emit().map_err(|e| AotError::Module(e.to_string()))?;
    Ok(ObjectArtifact {
        object,
        functions: ids.len(),
    })
}

/// Serialises per-prototype error sites: little-endian `u32` counts and lengths.
#[must_use]
pub fn encode_sites(all: &[Vec<ir::Site>]) -> Vec<u8> {
    let mut out = Vec::new();
    let w = |out: &mut Vec<u8>, n: usize| out.extend_from_slice(&(n as u32).to_le_bytes());
    w(&mut out, all.len());
    for sites in all {
        w(&mut out, sites.len());
        for s in sites {
            out.extend_from_slice(&s.pc.to_le_bytes());
            w(&mut out, s.slots.len());
            for slot in &s.slots {
                match slot {
                    None => out.push(0),
                    Some(t) => {
                        out.push(1);
                        w(&mut out, t.len());
                        out.extend_from_slice(t.as_bytes());
                    }
                }
            }
        }
    }
    out
}

/// Inverse of [`encode_sites`]; `None` on a truncated or malformed blob.
#[must_use]
pub fn decode_sites(blob: &[u8]) -> Option<Vec<Vec<ir::Site>>> {
    let mut pos = 0usize;
    let mut u32_at = |pos: &mut usize| -> Option<u32> {
        let b = blob.get(*pos..*pos + 4)?;
        *pos += 4;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let nprotos = u32_at(&mut pos)? as usize;
    let mut all = Vec::with_capacity(nprotos);
    for _ in 0..nprotos {
        let n = u32_at(&mut pos)? as usize;
        let mut sites = Vec::with_capacity(n);
        for _ in 0..n {
            let pc = u32_at(&mut pos)?;
            let ns = u32_at(&mut pos)? as usize;
            let mut slots = Vec::with_capacity(ns);
            for _ in 0..ns {
                let present = *blob.get(pos)?;
                pos += 1;
                slots.push(if present == 0 {
                    None
                } else {
                    let len = u32_at(&mut pos)? as usize;
                    let text = blob.get(pos..pos + len)?;
                    pos += len;
                    Some(String::from_utf8_lossy(text).into_owned())
                });
            }
            sites.push(ir::Site { pc, slots });
        }
        all.push(sites);
    }
    Some(all)
}

/// Type tag shared with generated code, re-exported for backends and tests.
pub const TAG_NUM: u64 = tag::NUM;
