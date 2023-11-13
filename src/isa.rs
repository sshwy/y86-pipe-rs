//! Instruction Set definition for Y86-64 Architecture */

use std::mem::transmute;

pub mod inst_code {
    pub const HALT: u8 = 0x0;
    pub const NOP: u8 = 0x1;
    pub const CMOVX: u8 = 0x2;
    pub const IRMOVQ: u8 = 0x3;
    pub const RMMOVQ: u8 = 0x4;
    pub const MRMOVQ: u8 = 0x5;
    pub const OPQ: u8 = 0x6;
    pub const JX: u8 = 0x7;
    pub const CALL: u8 = 0x8;
    pub const RET: u8 = 0x9;
    pub const PUSHQ: u8 = 0xa;
    pub const POPQ: u8 = 0xb;
}
pub mod reg_code {
    pub const RAX: u8 = 0;
    pub const RCX: u8 = 1;
    pub const RDX: u8 = 2;
    pub const RBX: u8 = 3;
    pub const RSP: u8 = 4;
    pub const RBP: u8 = 5;
    pub const RSI: u8 = 6;
    pub const RDI: u8 = 7;
    pub const R8: u8 = 8;
    pub const R9: u8 = 9;
    pub const R10: u8 = 0xa;
    pub const R11: u8 = 0xb;
    pub const R12: u8 = 0xc;
    pub const R13: u8 = 0xd;
    pub const R14: u8 = 0xe;
    pub const RNONE: u8 = 0xf;
}

pub mod op_code {
    pub const ADD: u8 = 0;
    pub const SUB: u8 = 1;
    pub const AND: u8 = 2;
    pub const XOR: u8 = 3;
}

pub mod cond_fn {
    pub const YES: u8 = 0;
    pub const LE: u8 = 1;
    pub const L: u8 = 2;
    pub const E: u8 = 3;
    pub const NE: u8 = 4;
    pub const GE: u8 = 5;
    pub const G: u8 = 6;
}
/// registers
#[derive(Debug, Clone, Copy)]
#[allow(clippy::upper_case_acronyms)]
pub enum Reg {
    RAX = reg_code::RAX as isize,
    RCX = reg_code::RCX as isize, // 1,
    RDX = reg_code::RDX as isize, // 2,
    RBX = reg_code::RBX as isize, // 3,
    RSP = reg_code::RSP as isize, // 4,
    RBP = reg_code::RBP as isize, // 5,
    RSI = reg_code::RSI as isize, // 6,
    RDI = reg_code::RDI as isize, // 7,
    R8 = reg_code::R8 as isize,   // 8,
    R9 = reg_code::R9 as isize,   // 9,
    R10 = reg_code::R10 as isize, // 0xa,
    R11 = reg_code::R11 as isize, // 0xb,
    R12 = reg_code::R12 as isize, // 0xc,
    R13 = reg_code::R13 as isize, // 0xd,
    R14 = reg_code::R14 as isize, // 0xe,
    RNONE = 0xf,
}

#[derive(Debug, Clone, Copy)]
#[allow(clippy::upper_case_acronyms)]
pub enum CondFn {
    /// jmp or rrmovq
    YES = 0,
    LE = 1,
    L = 2,
    E = 3,
    NE = 4,
    GE = 5,
    G = 6,
}

#[derive(Debug, Clone, Copy)]
#[allow(clippy::upper_case_acronyms)]
pub enum OpFn {
    ADD = 0,
    SUB = 1,
    AND = 2,
    XOR = 3,
}

impl From<u8> for OpFn {
    fn from(value: u8) -> Self {
        if value >= 4 {
            panic!("invalid op")
        }
        unsafe { transmute(value) }
    }
}

/// Address mode expression with optional displacement
#[derive(Debug, Clone, Copy)]
pub struct Addr(pub Option<u64>, pub Reg);

/// Y86 instructions
///
/// During assembling, the type of immediate can change
#[derive(Debug, Clone)]
#[allow(clippy::upper_case_acronyms)]
pub enum Inst<ImmType: Clone> {
    HALT,
    NOP,
    /// `rrmovq/cmovXX rA, rB`
    CMOVX(CondFn, Reg, Reg),
    /// `irmovq rB, V`
    IRMOVQ(Reg, ImmType),
    /// `rmmovq rA, D(rB)`
    RMMOVQ(Reg, Addr),
    /// `mrmovq D(rB), rA`
    MRMOVQ(Addr, Reg),
    OPQ(OpFn, Reg, Reg),
    JX(CondFn, ImmType),
    CALL(ImmType),
    RET,
    PUSHQ(Reg),
    POPQ(Reg),
    IOPQ(ImmType, Reg),
}

impl<ImmType: Clone> Inst<ImmType> {
    pub fn len(&self) -> usize {
        use Inst::*;
        match self {
            HALT | RET | NOP => 1,
            OPQ(_, _, _) | CMOVX(_, _, _) | PUSHQ(_) | POPQ(_) => 2,
            JX(_, _) | CALL(_) => 9,
            IRMOVQ(_, _) | RMMOVQ(_, _) | MRMOVQ(_, _) => 10,
            IOPQ(_, _) => todo!(),
        }
    }
    pub fn icode(&self) -> u8 {
        use inst_code::*;
        match &self {
            Inst::HALT => HALT,
            Inst::NOP => NOP,
            Inst::CMOVX(_, _, _) => CMOVX,
            Inst::IRMOVQ(_, _) => IRMOVQ,
            Inst::RMMOVQ(_, _) => RMMOVQ,
            Inst::MRMOVQ(_, _) => MRMOVQ,
            Inst::OPQ(_, _, _) => OPQ,
            Inst::JX(_, _) => JX,
            Inst::CALL(_) => CALL,
            Inst::RET => RET,
            Inst::PUSHQ(_) => PUSHQ,
            Inst::POPQ(_) => POPQ,
            Inst::IOPQ(_, _) => todo!(),
        }
    }
}
