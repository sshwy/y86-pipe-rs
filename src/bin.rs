//! This file provides binary representation of y86 instructions

use std::collections::BTreeMap;

use crate::parse::Rule;

/// registers
#[derive(Debug)]
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
}

impl From<&str> for Reg {
    fn from(value: &str) -> Self {
        match value {
            "%rax" => Reg::RAX,
            "%rbx" => Reg::RBX,
            "%rcx" => Reg::RCX,
            "%rdx" => Reg::RDX,
            "%rsi" => Reg::RSI,
            "%rdi" => Reg::RDI,
            "%rsp" => Reg::RSP,
            "%rbp" => Reg::RBP,
            "%r8" => Reg::R8,
            "%r9" => Reg::R9,
            "%r10" => Reg::R10,
            "%r11" => Reg::R11,
            "%r12" => Reg::R12,
            "%r13" => Reg::R13,
            "%r14" => Reg::R14,
            _ => panic!("invalid"),
        }
    }
}
#[derive(Debug)]
pub enum CondFn {
    LE = 1,
    L = 2,
    E = 3,
    NE = 4,
    GE = 5,
    G = 6,
}

impl From<&str> for CondFn {
    /// only check prefix
    fn from(value: &str) -> Self {
        if value.starts_with("le") {
            Self::LE
        } else if value.starts_with("l") {
            Self::L
        } else if value.starts_with("e") {
            Self::E
        } else if value.starts_with("ne") {
            Self::NE
        } else if value.starts_with("ge") {
            Self::GE
        } else if value.starts_with("g") {
            Self::G
        } else {
            panic!("invalid")
        }
    }
}

#[derive(Debug)]
pub enum OpFn {
    ADD = 0,
    SUB = 1,
    AND = 2,
    XOR = 3,
}

impl From<&str> for OpFn {
    fn from(value: &str) -> Self {
        if value.starts_with("andq") {
            Self::AND
        } else if value.starts_with("addq") {
            Self::ADD
        } else if value.starts_with("subq") {
            Self::SUB
        } else if value.starts_with("xorq") {
            Self::XOR
        } else {
            panic!("invalid")
        }
    }
}

/// Address mode expression with optional displacement
#[derive(Debug)]
pub struct Addr(Option<i64>, Reg);

impl From<pest::iterators::Pair<'_, Rule>> for Addr {
    fn from(value: pest::iterators::Pair<'_, Rule>) -> Self {
        let mut it = value.into_inner();
        let num_or_reg = it.next().unwrap();
        if num_or_reg.as_rule() == Rule::reg {
            // no displacement
            let reg = Reg::from(num_or_reg.as_str());
            Self(None, reg)
        } else {
            let s = num_or_reg.as_str();
            let num = if let Ok(r) = s.parse() {
                r
            } else {
                i64::from_str_radix(&s[2..], 16).unwrap()
            };
            let reg = Reg::from(it.next().unwrap().as_str());
            Self(Some(num), reg)
        }
    }
}

/// Immediate values (can be raw number or address of label)
#[derive(Debug)]
pub enum Imm {
    Num(i64),
    Label(String),
}

impl From<pest::iterators::Pair<'_, Rule>> for Imm {
    fn from(value: pest::iterators::Pair<'_, Rule>) -> Self {
        if value.as_rule() == Rule::label {
            Self::Label(value.as_str().to_string())
        } else {
            let s = value.as_str();
            let s = if s.starts_with("$") { &s[1..] } else { s };
            let num = if let Ok(r) = s.parse() {
                r
            } else {
                i64::from_str_radix(&s[2..], 16).unwrap()
            };
            Self::Num(num)
        }
    }
}

/// Y86 instructions
#[derive(Debug)]
pub enum Inst {
    HALT,
    NOP,
    /// `rrmovq/cmovXX rA, rB`
    CMOVX(Option<CondFn>, Reg, Reg),
    /// `irmovq rB, V`
    IRMOVQ(Reg, Imm),
    // `rmmovq rA, D(rB)`
    RMMOVQ(Reg, Addr),
    // `mrmovq D(rB), rA`
    MRMOVQ(Addr, Reg),
    OPQ(OpFn, Reg, Reg),
    JX(Option<CondFn>, Imm),
    CALL(Imm),
    RET,
    PUSHQ(Reg),
    POPQ(Reg),
    IOPQ(Imm, Reg),
}

#[derive(Debug)]
pub struct SourceInfo {
    pub addr: Option<u16>,
    pub inst: Option<Inst>,
    pub label: Option<String>,
    // width and data
    pub data: Option<(u8, u64)>,
    pub src: String,
}

/// object file
///
/// while y86 language support 64-bit address, we only consider address < 0x10000.
pub struct Object {
    pub binary: [u8; 1 << 16],
    /// basically labels
    pub symbols: BTreeMap<String, u16>,
    /// annotate each line with its address
    pub source: Vec<SourceInfo>,
}

impl Default for Object {
    fn default() -> Self {
        Self {
            binary: [0; 1 << 16],
            symbols: Default::default(),
            source: Default::default(),
        }
    }
}
