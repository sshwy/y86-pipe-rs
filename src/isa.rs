//! Instruction Set definition for Y86-64 Architecture */

/// registers
#[derive(Debug, Clone, Copy)]
pub enum Reg {
    RAX = 0,
    RCX = 1,
    RDX = 2,
    RBX = 3,
    RSP = 4,
    RBP = 5,
    RSI = 6,
    RDI = 7,
    R8 = 8,
    R9 = 9,
    R10 = 0xa,
    R11 = 0xb,
    R12 = 0xc,
    R13 = 0xd,
    R14 = 0xe,
    RNONE = 0xf,
}

#[derive(Debug, Clone, Copy)]
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
pub enum OpFn {
    ADD = 0,
    SUB = 1,
    AND = 2,
    XOR = 3,
}

/// Address mode expression with optional displacement
#[derive(Debug, Clone, Copy)]
pub struct Addr(pub Option<u64>, pub Reg);

/// Y86 instructions
///
/// During assembling, the type of immediate can change
#[derive(Debug, Clone)]
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
        match &self {
            Inst::HALT => 0x0,
            Inst::NOP => 0x1,
            Inst::CMOVX(_, _, _) => 0x2,
            Inst::IRMOVQ(_, _) => 0x3,
            Inst::RMMOVQ(_, _) => 0x4,
            Inst::MRMOVQ(_, _) => 0x5,
            Inst::OPQ(_, _, _) => 0x6,
            Inst::JX(_, _) => 0x7,
            Inst::CALL(_) => 0x8,
            Inst::RET => 0x9,
            Inst::PUSHQ(_) => 0xa,
            Inst::POPQ(_) => 0xb,
            Inst::IOPQ(_, _) => todo!(),
        }
    }
}
