//! Instruction Set definition for Y86-64 Architecture */

use std::mem::transmute;

macro_rules! define_code {
    {
        @mod $modname:ident;
        @type $typ:ty;
        $( $cname:ident = $cval:expr; )*
    } => {
        pub mod $modname {
            $(pub const $cname : $typ = $cval; )*
            #[allow(unused)]
            pub fn name_of(code: $typ) -> &'static str {
                match code {
                    $($cname => stringify!($cname), )*
                    _ => "no name"
                }
            }
        }
    };
}

define_code!{
    @mod inst_code;
    @type u8;
    HALT = 0x0;
    NOP = 0x1;
    CMOVX = 0x2;
    IRMOVQ = 0x3;
    RMMOVQ = 0x4;
    MRMOVQ = 0x5;
    OPQ = 0x6;
    JX = 0x7;
    CALL = 0x8;
    RET = 0x9;
    PUSHQ = 0xa;
    POPQ = 0xb;
}

define_code!{
    @mod reg_code;
    @type u8;
    RAX = 0;
    RCX = 1;
    RDX = 2;
    RBX = 3;
    RSP = 4;
    RBP = 5;
    RSI = 6;
    RDI = 7;
    R8 = 8;
    R9 = 9;
    R10 = 0xa;
    R11 = 0xb;
    R12 = 0xc;
    R13 = 0xd;
    R14 = 0xe;
    RNONE = 0xf;
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
