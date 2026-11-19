//! Typed IR of one Lua function: basic blocks over the Lua registers, with a per-register type
//! lattice that lets the backends drop dynamic checks.
//!
//! The IR keeps the register machine of Lua 5.2 (frames, upvalues and varargs have the same
//! meaning in the native runtime) but removes everything positional: jumps become block
//! edges, `RK` operands become explicit, conditional jumps fuse with their `JMP`, and the
//! implicit "top" of multi-value sequences becomes an explicit [`Count::ToTop`].

use std::collections::BTreeSet;

use crate::bytecode::{Constant, Prototype, decode_instruction};

/// Set of possible dynamic types of a register, as a bit set.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Ty(pub u8);

impl Ty {
    /// Nothing known yet (unreached).
    pub const NONE: Self = Self(0);
    /// `nil`.
    pub const NIL: Self = Self(1);
    /// Boolean.
    pub const BOOL: Self = Self(2);
    /// Number.
    pub const NUM: Self = Self(4);
    /// String.
    pub const STR: Self = Self(8);
    /// Table.
    pub const TABLE: Self = Self(16);
    /// Function.
    pub const FUNC: Self = Self(32);
    /// Anything.
    pub const ANY: Self = Self(127);

    /// Union.
    #[must_use]
    pub const fn join(self, o: Self) -> Self {
        Self(self.0 | o.0)
    }

    /// `true` when the set is exactly `o`.
    #[must_use]
    pub const fn is(self, o: Self) -> bool {
        self.0 == o.0 && self.0 != 0
    }
}

/// Register index.
pub type Reg = u8;

/// A register or a constant (`RK`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Opnd {
    /// Register.
    R(Reg),
    /// Constant pool index.
    K(u32),
}

/// A count of values that is either fixed or "up to the top mark".
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Count {
    /// Exactly this many.
    Fixed(u32),
    /// Everything up to the top left by the previous multi-value producer.
    ToTop,
}

/// Block index.
pub type BlockId = usize;

/// Non-terminating operation.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Op {
    /// `R[a] = R[b]`.
    Move { a: Reg, b: Reg },
    /// `R[a] = K[k]`.
    LoadK { a: Reg, k: u32 },
    /// `R[a] = v`.
    LoadBool { a: Reg, v: bool },
    /// `R[a..=a+n] = nil`.
    LoadNil { a: Reg, n: u32 },
    /// `R[a] = Up[u]`.
    GetUpval { a: Reg, u: u32 },
    /// `Up[u] = R[a]`.
    SetUpval { a: Reg, u: u32 },
    /// `R[a] = Up[u][key]`.
    GetTabUp { a: Reg, u: u32, key: Opnd },
    /// `Up[u][key] = val`.
    SetTabUp { u: u32, key: Opnd, val: Opnd },
    /// `R[a] = R[b][key]`.
    GetTable { a: Reg, b: Reg, key: Opnd },
    /// `R[a][key] = val`.
    SetTable { a: Reg, key: Opnd, val: Opnd },
    /// `R[a] = {}`.
    NewTable { a: Reg, narr: u32, nhash: u32 },
    /// `R[a+1] = R[b]; R[a] = R[b][key]`.
    SelfOp { a: Reg, b: Reg, key: Opnd },
    /// `R[a] = b op c` (`op` per [`crate::rt::state::arith`] codes); `nums` when both operands
    /// are statically numbers.
    Arith { op: u32, a: Reg, b: Opnd, c: Opnd, nums: bool },
    /// `R[a] = -R[b]`.
    Unm { a: Reg, b: Reg, num: bool },
    /// `R[a] = not R[b]`.
    Not { a: Reg, b: Reg },
    /// `R[a] = #R[b]`.
    Len { a: Reg, b: Reg },
    /// `R[a] = R[b] .. ... .. R[c]`.
    Concat { a: Reg, b: Reg, c: Reg },
    /// Closes the upvalues of registers `>= from`.
    Close { from: Reg },
    /// `R[a] = closure(proto[idx])`.
    Closure { a: Reg, idx: u32 },
    /// `R[a..] = R[a](R[a+1..])`.
    Call { a: Reg, args: Count, rets: Count },
    /// `R[a][start+1..] = R[a+1..]`.
    SetList { a: Reg, n: Count, start: u32 },
    /// `R[a..] = ...`.
    VarArg { a: Reg, n: Count },
    /// Prepares a numeric `for`: normalises `R[a..a+3]` and subtracts the step.
    ForPrep { a: Reg },
    /// Generic `for` call: `R[a+3..a+3+n] = R[a](R[a+1], R[a+2])`.
    TForCall { a: Reg, n: u32 },
}

/// Condition of a conditional edge.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Cond {
    /// `(b == c) == expect`.
    Eq { b: Opnd, c: Opnd, expect: bool, nums: bool },
    /// `(b < c) == expect`.
    Lt { b: Opnd, c: Opnd, expect: bool, nums: bool },
    /// `(b <= c) == expect`.
    Le { b: Opnd, c: Opnd, expect: bool, nums: bool },
    /// `truthy(R[a]) == expect`.
    Test { a: Reg, expect: bool },
}

/// Block terminator.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Term {
    /// Unconditional edge.
    Jmp(BlockId),
    /// `then` when the condition holds, else `els`.
    Br { cond: Cond, then: BlockId, els: BlockId },
    /// Numeric `for` step: `R[a] += R[a+2]`, continue to `body` (with `R[a+3] = R[a]`) while in
    /// range, else `exit`.
    ForLoop { a: Reg, body: BlockId, exit: BlockId },
    /// Generic `for` step: if `R[a+1] ~= nil` then `R[a] = R[a+1]` and `body`, else `exit`.
    TForLoop { a: Reg, body: BlockId, exit: BlockId },
    /// Returns `R[a..a+n]`.
    Return { a: Reg, n: Count },
}

/// A basic block.
#[derive(Clone, Debug)]
pub struct Block {
    /// Index of the first bytecode instruction (diagnostics).
    pub pc: usize,
    /// Straight-line operations.
    pub ops: Vec<Op>,
    /// Bytecode index of each operation, for error descriptions.
    pub pcs: Vec<u32>,
    /// Edge out.
    pub term: Term,
}

/// A lowered function.
#[derive(Clone, Debug)]
pub struct Func {
    /// Fixed parameters.
    pub num_params: u32,
    /// Accepts `...`.
    pub is_vararg: bool,
    /// Registers used by the frame.
    pub max_stack: u32,
    /// Number of upvalues.
    pub num_upvals: u32,
    /// Constant pool.
    pub consts: Vec<Constant>,
    /// Nested function count.
    pub num_children: u32,
    /// Blocks; block 0 is the entry.
    pub blocks: Vec<Block>,
    /// `true` when some nested closure captures a register of this frame, so returns must
    /// close upvalues.
    pub captures_locals: bool,
}

/// Lowering failure: malformed bytecode.
#[derive(Debug, thiserror::Error)]
#[error("malformed bytecode at pc {pc}: {reason}")]
pub struct LowerError {
    /// Instruction index.
    pub pc: usize,
    /// Cause.
    pub reason: String,
}

const BITRK: u32 = 1 << 8;

fn err<T>(pc: usize, reason: &str) -> Result<T, LowerError> {
    Err(LowerError {
        pc,
        reason: reason.to_string(),
    })
}

struct Lower<'a> {
    p: &'a Prototype,
    max: u32,
}

impl Lower<'_> {
    fn reg(&self, pc: usize, r: u32) -> Result<Reg, LowerError> {
        if r >= self.max.max(2) {
            return err(pc, "register out of frame");
        }
        Ok(r as Reg)
    }

    fn rk(&self, pc: usize, x: u32) -> Result<Opnd, LowerError> {
        if x & BITRK != 0 {
            let k = x & !BITRK;
            if k as usize >= self.p.constants.len() {
                return err(pc, "constant out of range");
            }
            Ok(Opnd::K(k))
        } else {
            Ok(Opnd::R(self.reg(pc, x)?))
        }
    }

    fn upval(&self, pc: usize, u: u32) -> Result<u32, LowerError> {
        if u as usize >= self.p.upvalues.len() {
            return err(pc, "upvalue out of range");
        }
        Ok(u)
    }
}

fn target(pc: usize, sbx: i32, len: usize) -> Result<usize, LowerError> {
    let t = pc as i64 + 1 + i64::from(sbx);
    if t < 0 || t as usize >= len {
        return err(pc, "jump out of range");
    }
    Ok(t as usize)
}

/// Lowers one prototype to IR.
///
/// # Errors
/// [`LowerError`] when an operand or a jump target is out of range.
pub fn lower(p: &Prototype) -> Result<Func, LowerError> {
    let code = &p.code;
    let len = code.len();
    if len == 0 {
        return err(0, "empty function");
    }
    let lw = Lower {
        p,
        max: u32::from(p.max_stack_size),
    };
    // Leaders.
    let mut leaders: BTreeSet<usize> = BTreeSet::from([0]);
    let mut pc = 0;
    while pc < len {
        let i = decode_instruction(code[pc]);
        match i.opcode {
            23 => {
                leaders.insert(target(pc, i.sbx, len)?);
                leaders.insert(pc + 1);
            }
            24..=28 => {
                // EQ LT LE TEST TESTSET: the JMP at pc+1 owns its block.
                leaders.insert(pc + 1);
                leaders.insert(pc + 2);
            }
            3 if i.c != 0 => {
                leaders.insert(pc + 2);
                leaders.insert(pc + 1);
            }
            32 | 35 => {
                leaders.insert(target(pc, i.sbx, len)?);
                leaders.insert(pc + 1);
            }
            33 => {
                leaders.insert(target(pc, i.sbx, len)?);
                leaders.insert(pc + 1);
            }
            30 | 31 => {
                leaders.insert(pc + 1);
            }
            2 | 36 if (i.opcode == 2 || i.c == 0) => {
                pc += 1;
            }
            _ => {}
        }
        pc += 1;
    }
    leaders.retain(|l| *l < len);
    let starts: Vec<usize> = leaders.iter().copied().collect();
    let block_of = |at: usize| -> Result<BlockId, LowerError> {
        starts.binary_search(&at).map_err(|_| LowerError {
            pc: at,
            reason: "jump to a non-leader".to_string(),
        })
    };
    let mut blocks: Vec<Block> = Vec::with_capacity(starts.len() + 4);
    for &s in &starts {
        blocks.push(Block {
            pc: s,
            ops: Vec::new(),
            pcs: Vec::new(),
            term: Term::Jmp(0),
        });
    }
    let mut captured_any = false;
    for child in &p.protos {
        captured_any |= child.upvalues.iter().any(|u| u.in_stack);
    }
    for (bi, &start) in starts.iter().enumerate() {
        let end = starts.get(bi + 1).copied().unwrap_or(len);
        let mut pc = start;
        let mut term: Option<Term> = None;
        while pc < end {
            let i = decode_instruction(code[pc]);
            let (a, b, c) = (i.a, i.b, i.c);
            let ipc = pc as u32;
            macro_rules! push {
                ($op:expr) => {{
                    let op = $op;
                    blocks[bi].ops.push(op);
                    blocks[bi].pcs.push(ipc);
                }};
            }
            match i.opcode {
                0 => push!(Op::Move { a: lw.reg(pc, a)?, b: lw.reg(pc, b)? }),
                1 | 2 => {
                    let k = if i.opcode == 1 {
                        i.bx
                    } else {
                        if pc + 1 >= len {
                            return err(pc, "LOADKX without EXTRAARG");
                        }
                        pc += 1;
                        decode_instruction(code[pc]).ax
                    };
                    if k as usize >= p.constants.len() {
                        return err(pc, "constant out of range");
                    }
                    push!(Op::LoadK { a: lw.reg(pc, a)?, k });
                }
                3 => {
                    push!(Op::LoadBool { a: lw.reg(pc, a)?, v: b != 0 });
                    if c != 0 {
                        term = Some(Term::Jmp(block_of(pc + 2)?));
                    }
                }
                4 => {
                    lw.reg(pc, a + b)?;
                    push!(Op::LoadNil { a: lw.reg(pc, a)?, n: b });
                }
                5 => push!(Op::GetUpval { a: lw.reg(pc, a)?, u: lw.upval(pc, b)? }),
                6 => push!(Op::GetTabUp {
                    a: lw.reg(pc, a)?,
                    u: lw.upval(pc, b)?,
                    key: lw.rk(pc, c)?
                }),
                7 => push!(Op::GetTable {
                    a: lw.reg(pc, a)?,
                    b: lw.reg(pc, b)?,
                    key: lw.rk(pc, c)?
                }),
                8 => push!(Op::SetTabUp {
                    u: lw.upval(pc, a)?,
                    key: lw.rk(pc, b)?,
                    val: lw.rk(pc, c)?
                }),
                9 => push!(Op::SetUpval { a: lw.reg(pc, a)?, u: lw.upval(pc, b)? }),
                10 => push!(Op::SetTable {
                    a: lw.reg(pc, a)?,
                    key: lw.rk(pc, b)?,
                    val: lw.rk(pc, c)?
                }),
                11 => push!(Op::NewTable {
                    a: lw.reg(pc, a)?,
                    narr: fb2int(b),
                    nhash: fb2int(c)
                }),
                12 => {
                    lw.reg(pc, a + 1)?;
                    push!(Op::SelfOp {
                        a: lw.reg(pc, a)?,
                        b: lw.reg(pc, b)?,
                        key: lw.rk(pc, c)?
                    });
                }
                13..=18 => push!(Op::Arith {
                    op: i.opcode as u32 - 13,
                    a: lw.reg(pc, a)?,
                    b: lw.rk(pc, b)?,
                    c: lw.rk(pc, c)?,
                    nums: false
                }),
                19 => push!(Op::Unm { a: lw.reg(pc, a)?, b: lw.reg(pc, b)?, num: false }),
                20 => push!(Op::Not { a: lw.reg(pc, a)?, b: lw.reg(pc, b)? }),
                21 => push!(Op::Len { a: lw.reg(pc, a)?, b: lw.reg(pc, b)? }),
                22 => {
                    if c < b {
                        return err(pc, "CONCAT range");
                    }
                    push!(Op::Concat {
                        a: lw.reg(pc, a)?,
                        b: lw.reg(pc, b)?,
                        c: lw.reg(pc, c)?
                    });
                }
                23 => {
                    if a != 0 {
                        push!(Op::Close { from: lw.reg(pc, a - 1)? });
                    }
                    term = Some(Term::Jmp(block_of(target(pc, i.sbx, len)?)?));
                }
                24..=26 => {
                    let (bo, co) = (lw.rk(pc, b)?, lw.rk(pc, c)?);
                    let expect = a != 0;
                    let cond = match i.opcode {
                        24 => Cond::Eq { b: bo, c: co, expect, nums: false },
                        25 => Cond::Lt { b: bo, c: co, expect, nums: false },
                        _ => Cond::Le { b: bo, c: co, expect, nums: false },
                    };
                    term = Some(Term::Br {
                        cond,
                        then: block_of(pc + 1)?,
                        els: block_of(pc + 2)?,
                    });
                }
                27 => {
                    term = Some(Term::Br {
                        cond: Cond::Test { a: lw.reg(pc, a)?, expect: c != 0 },
                        then: block_of(pc + 1)?,
                        els: block_of(pc + 2)?,
                    });
                }
                28 => {
                    // TESTSET: the move happens on the taken edge, through a synthetic block.
                    let (ra, rb) = (lw.reg(pc, a)?, lw.reg(pc, b)?);
                    let synth = blocks.len();
                    blocks.push(Block {
                        pc,
                        ops: vec![Op::Move { a: ra, b: rb }],
                        pcs: vec![pc as u32],
                        term: Term::Jmp(block_of(pc + 1)?),
                    });
                    term = Some(Term::Br {
                        cond: Cond::Test { a: rb, expect: c != 0 },
                        then: synth,
                        els: block_of(pc + 2)?,
                    });
                }
                29 | 30 => {
                    let ra = lw.reg(pc, a)?;
                    let args = if b == 0 { Count::ToTop } else { Count::Fixed(b - 1) };
                    if let Count::Fixed(n) = args {
                        lw.reg(pc, a + n)?;
                    }
                    if i.opcode == 29 {
                        let rets = if c == 0 { Count::ToTop } else { Count::Fixed(c - 1) };
                        push!(Op::Call { a: ra, args, rets });
                    } else {
                        push!(Op::Call { a: ra, args, rets: Count::ToTop });
                        term = Some(Term::Return { a: ra, n: Count::ToTop });
                    }
                }
                31 => {
                    let n = if b == 0 { Count::ToTop } else { Count::Fixed(b - 1) };
                    term = Some(Term::Return { a: lw.reg(pc, a)?, n });
                }
                32 => {
                    term = Some(Term::ForLoop {
                        a: lw.reg(pc, a + 3)? - 3,
                        body: block_of(target(pc, i.sbx, len)?)?,
                        exit: block_of(pc + 1)?,
                    });
                }
                33 => {
                    push!(Op::ForPrep { a: lw.reg(pc, a + 3)? - 3 });
                    term = Some(Term::Jmp(block_of(target(pc, i.sbx, len)?)?));
                }
                34 => {
                    lw.reg(pc, a + 2 + c)?;
                    push!(Op::TForCall { a: lw.reg(pc, a)?, n: c });
                }
                35 => {
                    term = Some(Term::TForLoop {
                        a: lw.reg(pc, a + 1)? - 1,
                        body: block_of(target(pc, i.sbx, len)?)?,
                        exit: block_of(pc + 1)?,
                    });
                }
                36 => {
                    let n = if b == 0 { Count::ToTop } else { Count::Fixed(b) };
                    let block = if c == 0 {
                        if pc + 1 >= len {
                            return err(pc, "SETLIST without EXTRAARG");
                        }
                        pc += 1;
                        decode_instruction(code[pc]).ax
                    } else {
                        c - 1
                    };
                    push!(Op::SetList { a: lw.reg(pc, a)?, n, start: block * 50 });
                }
                37 => {
                    if i.bx as usize >= p.protos.len() {
                        return err(pc, "closure index out of range");
                    }
                    push!(Op::Closure { a: lw.reg(pc, a)?, idx: i.bx });
                }
                38 => {
                    let n = if b == 0 { Count::ToTop } else { Count::Fixed(b - 1) };
                    push!(Op::VarArg { a: lw.reg(pc, a)?, n });
                }
                _ => return err(pc, "unsupported opcode"),
            }
            pc += 1;
            if term.is_some() {
                break;
            }
        }
        blocks[bi].term = match term {
            Some(t) => t,
            None => {
                if pc >= len {
                    return err(pc, "falls off the end of the function");
                }
                Term::Jmp(block_of(pc)?)
            }
        };
    }
    let mut func = Func {
        num_params: u32::from(p.num_params),
        is_vararg: p.is_vararg != 0,
        max_stack: u32::from(p.max_stack_size).max(2),
        num_upvals: p.upvalues.len() as u32,
        consts: p.constants.clone(),
        num_children: p.protos.len() as u32,
        blocks,
        captures_locals: captured_any,
    };
    infer(&mut func, p);
    Ok(func)
}

/// Decodes the floating-point byte of `NEWTABLE` size hints.
fn fb2int(x: u32) -> u32 {
    let e = (x >> 3) & 31;
    if e == 0 { x } else { ((x & 7) + 8) << (e - 1) }
}

/// Registers captured by nested closures: their type cannot be tracked.
fn captured_regs(p: &Prototype) -> Vec<bool> {
    let mut v = vec![false; 256];
    for child in &p.protos {
        for u in &child.upvalues {
            if u.in_stack {
                v[usize::from(u.index)] = true;
            }
        }
    }
    v
}

type State = Vec<Ty>;

fn opnd_ty(o: Opnd, st: &State, consts: &[Constant]) -> Ty {
    match o {
        Opnd::R(r) => st[usize::from(r)],
        Opnd::K(k) => match &consts[k as usize] {
            Constant::Nil => Ty::NIL,
            Constant::Boolean(_) => Ty::BOOL,
            Constant::Number(_) => Ty::NUM,
            Constant::String(_) => Ty::STR,
        },
    }
}

fn set(st: &mut State, cap: &[bool], r: Reg, t: Ty) {
    st[usize::from(r)] = if cap[usize::from(r)] { Ty::ANY } else { t };
}

fn clobber_from(st: &mut State, from: Reg) {
    for t in st.iter_mut().skip(usize::from(from)) {
        *t = Ty::ANY;
    }
}

/// Forward dataflow of register types, annotating `nums` flags.
fn infer(f: &mut Func, p: &Prototype) {
    let cap = captured_regs(p);
    let n = f.max_stack as usize + 4;
    let nb = f.blocks.len();
    let mut entry: Vec<Option<State>> = vec![None; nb];
    entry[0] = Some(vec![Ty::ANY; n]);
    let mut work = vec![0usize];
    // Run to a fixpoint; annotation happens in a final pass over the stable entry states.
    let transfer = |f: &Func, bi: usize, st: &mut State| {
        for op in &f.blocks[bi].ops {
            apply(op, st, &cap, &f.consts);
        }
    };
    while let Some(bi) = work.pop() {
        let Some(mut st) = entry[bi].clone() else {
            continue;
        };
        transfer(f, bi, &mut st);
        let mut succs: Vec<(BlockId, State)> = Vec::new();
        match f.blocks[bi].term {
            Term::Jmp(t) => succs.push((t, st)),
            Term::Br { then, els, .. } => {
                succs.push((then, st.clone()));
                succs.push((els, st));
            }
            Term::ForLoop { a, body, exit } => {
                let mut s = st.clone();
                for k in 0..4 {
                    set(&mut s, &cap, a + k, Ty::NUM);
                }
                succs.push((body, s));
                succs.push((exit, st));
            }
            Term::TForLoop { a, body, exit } => {
                let mut s = st.clone();
                set(&mut s, &cap, a, Ty::ANY);
                succs.push((body, s));
                succs.push((exit, st));
            }
            Term::Return { .. } => {}
        }
        for (t, s) in succs {
            match &mut entry[t] {
                None => {
                    entry[t] = Some(s);
                    work.push(t);
                }
                Some(old) => {
                    let mut changed = false;
                    for (o, n) in old.iter_mut().zip(&s) {
                        let j = o.join(*n);
                        if j != *o {
                            *o = j;
                            changed = true;
                        }
                    }
                    if changed {
                        work.push(t);
                    }
                }
            }
        }
    }
    // Annotation pass.
    for bi in 0..nb {
        let Some(mut st) = entry[bi].clone() else {
            continue;
        };
        let consts = f.consts.clone();
        let mut ops = std::mem::take(&mut f.blocks[bi].ops);
        for op in &mut ops {
            match op {
                Op::Arith { b, c, nums, .. } => {
                    *nums = opnd_ty(*b, &st, &consts).is(Ty::NUM) && opnd_ty(*c, &st, &consts).is(Ty::NUM);
                }
                Op::Unm { b, num, .. } => *num = st[usize::from(*b)].is(Ty::NUM),
                _ => {}
            }
            apply(op, &mut st, &cap, &consts);
        }
        f.blocks[bi].ops = ops;
        if let Term::Br { cond, .. } = &mut f.blocks[bi].term {
            match cond {
                Cond::Eq { b, c, nums, .. }
                | Cond::Lt { b, c, nums, .. }
                | Cond::Le { b, c, nums, .. } => {
                    *nums = opnd_ty(*b, &st, &consts).is(Ty::NUM) && opnd_ty(*c, &st, &consts).is(Ty::NUM);
                }
                Cond::Test { .. } => {}
            }
        }
    }
}

fn apply(op: &Op, st: &mut State, cap: &[bool], consts: &[Constant]) {
    match *op {
        Op::Move { a, b } => {
            let t = st[usize::from(b)];
            set(st, cap, a, t);
        }
        Op::LoadK { a, k } => {
            let t = opnd_ty(Opnd::K(k), st, consts);
            set(st, cap, a, t);
        }
        Op::LoadBool { a, .. } | Op::Not { a, .. } => set(st, cap, a, Ty::BOOL),
        Op::LoadNil { a, n } => {
            for k in 0..=n {
                set(st, cap, a + k as Reg, Ty::NIL);
            }
        }
        Op::NewTable { a, .. } => set(st, cap, a, Ty::TABLE),
        Op::Closure { a, .. } => set(st, cap, a, Ty::FUNC),
        Op::Arith { a, b, c, .. } => {
            let nums = opnd_ty(b, st, consts).is(Ty::NUM) && opnd_ty(c, st, consts).is(Ty::NUM);
            set(st, cap, a, if nums { Ty::NUM } else { Ty::ANY });
        }
        Op::Unm { a, b, .. } => {
            let t = if st[usize::from(b)].is(Ty::NUM) { Ty::NUM } else { Ty::ANY };
            set(st, cap, a, t);
        }
        Op::Len { a, .. } => set(st, cap, a, Ty::ANY),
        Op::Concat { a, .. } => set(st, cap, a, Ty::ANY),
        Op::GetUpval { a, .. }
        | Op::GetTabUp { a, .. }
        | Op::GetTable { a, .. } => set(st, cap, a, Ty::ANY),
        Op::SelfOp { a, .. } => {
            set(st, cap, a, Ty::ANY);
            set(st, cap, a + 1, Ty::ANY);
        }
        Op::SetUpval { .. } | Op::SetTabUp { .. } | Op::SetTable { .. } | Op::Close { .. } | Op::SetList { .. } => {}
        Op::Call { a, .. } | Op::VarArg { a, .. } => clobber_from(st, a),
        Op::ForPrep { a } => {
            for k in 0..3 {
                set(st, cap, a + k, Ty::NUM);
            }
        }
        Op::TForCall { a, n } => {
            for k in 0..n.max(1) {
                set(st, cap, a + 3 + k as Reg, Ty::ANY);
            }
        }
    }
    // Calls and metamethods may write captured registers through upvalues.
    if matches!(
        op,
        Op::Call { .. }
            | Op::TForCall { .. }
            | Op::GetTable { .. }
            | Op::SetTable { .. }
            | Op::GetTabUp { .. }
            | Op::SetTabUp { .. }
            | Op::Arith { .. }
            | Op::Concat { .. }
            | Op::Len { .. }
            | Op::SelfOp { .. }
    ) {
        for (r, c) in cap.iter().enumerate() {
            if *c && r < st.len() {
                st[r] = Ty::ANY;
            }
        }
    }
}

// ------------------------------------------------------------------------------ error sites

/// Description of the operands of one operation that can raise a type error, in the shape of
/// `luaG_typeerror`'s `varinfo`: `global 'x'`, `field 'y'`, `method 'm'`, `upvalue 'u'`,
/// `constant 'c'`, `local 'l'`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Site {
    /// Bytecode index of the operation.
    pub pc: u32,
    /// One entry per blamable operand.
    pub slots: Vec<Option<String>>,
}

fn sets_a(op: u8) -> bool {
    !matches!(op, 8 | 9 | 10 | 23 | 24 | 25 | 26 | 27 | 31 | 34 | 36 | 39)
}

fn find_set_reg(p: &Prototype, lastpc: usize, reg: u32) -> Option<usize> {
    let mut setreg: Option<usize> = None;
    let mut jmptarget = 0usize;
    let filter = |pc: usize, jt: usize| if pc < jt { None } else { Some(pc) };
    for pc in 0..lastpc {
        let i = decode_instruction(p.code[pc]);
        let a = i.a;
        match i.opcode {
            4 => {
                if a <= reg && reg <= a + i.b {
                    setreg = filter(pc, jmptarget);
                }
            }
            34 => {
                if reg >= a + 2 {
                    setreg = filter(pc, jmptarget);
                }
            }
            29 | 30 => {
                if reg >= a {
                    setreg = filter(pc, jmptarget);
                }
            }
            23 => {
                let dest = pc as i64 + 1 + i64::from(i.sbx);
                if (pc as i64) < dest && dest <= lastpc as i64 && dest as usize > jmptarget {
                    jmptarget = dest as usize;
                }
            }
            op => {
                if sets_a(op) && reg == a {
                    setreg = filter(pc, jmptarget);
                }
            }
        }
    }
    setreg
}

fn local_name(p: &Prototype, reg: u32, pc: usize) -> Option<&str> {
    let mut n = reg + 1;
    for v in &p.loc_vars {
        if v.start_pc as usize > pc {
            break;
        }
        if pc < v.end_pc as usize {
            n -= 1;
            if n == 0 {
                return Some(&v.name);
            }
        }
    }
    None
}

fn upval_name(p: &Prototype, u: u32) -> String {
    p.upvalue_names
        .get(u as usize)
        .filter(|n| !n.is_empty())
        .cloned()
        .unwrap_or_else(|| "?".to_string())
}

fn const_name(p: &Prototype, k: u32) -> Option<String> {
    match p.constants.get(k as usize) {
        Some(Constant::String(s)) => Some(String::from_utf8_lossy(s).into_owned()),
        _ => None,
    }
}

fn key_name(p: &Prototype, pc: usize, c: u32) -> String {
    if c & BITRK != 0 {
        return const_name(p, c & !BITRK).unwrap_or_else(|| "?".to_string());
    }
    match get_obj_name(p, pc, c) {
        Some(("constant", name)) => name,
        _ => "?".to_string(),
    }
}

fn get_obj_name(p: &Prototype, lastpc: usize, reg: u32) -> Option<(&'static str, String)> {
    if let Some(n) = local_name(p, reg, lastpc) {
        return Some(("local", n.to_string()));
    }
    let pc = find_set_reg(p, lastpc, reg)?;
    let i = decode_instruction(p.code[pc]);
    match i.opcode {
        0 if i.b < i.a => get_obj_name(p, pc, i.b),
        6 | 7 => {
            let name = key_name(p, pc, i.c);
            let table_name = if i.opcode == 7 {
                local_name(p, i.b, pc).map(str::to_string)
            } else {
                Some(upval_name(p, i.b))
            };
            let kind = if table_name.as_deref() == Some("_ENV") { "global" } else { "field" };
            Some((kind, name))
        }
        5 => Some(("upvalue", upval_name(p, i.b))),
        1 | 2 => {
            let k = if i.opcode == 1 {
                i.bx
            } else {
                decode_instruction(*p.code.get(pc + 1)?).ax
            };
            const_name(p, k).map(|n| ("constant", n))
        }
        12 => Some(("method", key_name(p, pc, i.c))),
        _ => None,
    }
}

fn reg_desc(p: &Prototype, pc: usize, reg: u32) -> Option<String> {
    get_obj_name(p, pc, reg).map(|(kind, name)| format!("{kind} '{name}'"))
}

fn rk_desc(p: &Prototype, pc: usize, x: u32) -> Option<String> {
    if x & BITRK != 0 { None } else { reg_desc(p, pc, x) }
}

/// Computes the error descriptions of every operation of `p` that can raise a type error.
#[must_use]
pub fn sites(p: &Prototype) -> Vec<Site> {
    let mut out = Vec::new();
    for pc in 0..p.code.len() {
        let i = decode_instruction(p.code[pc]);
        let slots = match i.opcode {
            6 => vec![Some(format!("upvalue '{}'", upval_name(p, i.b)))],
            7 | 12 => vec![reg_desc(p, pc, i.b)],
            8 => vec![Some(format!("upvalue '{}'", upval_name(p, i.a)))],
            10 => vec![reg_desc(p, pc, i.a)],
            13..=18 => vec![rk_desc(p, pc, i.b), rk_desc(p, pc, i.c)],
            19 | 21 => vec![reg_desc(p, pc, i.b)],
            22 => (i.b..=i.c).map(|r| reg_desc(p, pc, r)).collect(),
            29 | 30 => vec![reg_desc(p, pc, i.a)],
            34 => vec![reg_desc(p, pc, i.a + 3)],
            _ => continue,
        };
        if slots.iter().any(Option::is_some) {
            out.push(Site { pc: pc as u32, slots });
        }
    }
    out
}
