//! This file provides binary representation of y86 instructions

use std::{collections::BTreeMap, fmt::Display};

use crate::asm::Rule;
use crate::isa::{self, Addr, CondFn, OpFn, Reg};

pub const BIN_SIZE: usize = 1 << 16;
pub type SymbolMap = BTreeMap<String, u64>;

// little endian
pub fn get_u64(binary: &[u8]) -> u64 {
    let mut res = 0;
    for (i, byte) in binary.iter().enumerate().take(8) {
        res += (*byte as u64) << (i * 8);
    }
    // for i in 0..8 {
    //     res += (binary[i] as u64) << (i * 8);
    // }
    res
}
pub fn put_u64(binary: &mut [u8], val: u64) {
    for (i, byte) in binary.iter_mut().enumerate().take(8) {
        *byte = (val >> (i * 8)) as u8;
    }
}

impl From<pest::iterators::Pair<'_, Rule>> for Reg {
    fn from(value: pest::iterators::Pair<'_, Rule>) -> Self {
        match value.as_str() {
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

impl From<&str> for CondFn {
    /// only check prefix
    fn from(value: &str) -> Self {
        if value.starts_with("le") {
            Self::LE
        } else if value.starts_with('l') {
            Self::L
        } else if value.starts_with('e') {
            Self::E
        } else if value.starts_with("ne") {
            Self::NE
        } else if value.starts_with("ge") {
            Self::GE
        } else if value.starts_with('g') {
            Self::G
        } else {
            Self::YES
        }
    }
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

impl From<pest::iterators::Pair<'_, Rule>> for Addr {
    fn from(value: pest::iterators::Pair<'_, Rule>) -> Self {
        let mut it = value.into_inner();
        let num_or_reg = it.next().unwrap();
        if num_or_reg.as_rule() == Rule::reg {
            // no displacement
            let reg = isa::Reg::from(num_or_reg);
            Self(None, reg)
        } else {
            let s = num_or_reg.as_str();
            let num = if let Ok(r) = s.parse() {
                r
            } else {
                i64::from_str_radix(&s[2..], 16).unwrap()
            };
            let reg = Reg::from(it.next().unwrap());
            Self(Some(num as u64), reg)
        }
    }
}

/// Immediate values (can be raw number or address of label)
#[derive(Debug, Clone)]
pub enum Imm {
    Num(i64),
    Label(String),
}

impl Imm {
    fn desymbol(&self, sym: &SymbolMap) -> u64 {
        match self {
            Imm::Num(n) => *n as u64,
            Imm::Label(label) => sym[label],
        }
    }
}

impl From<pest::iterators::Pair<'_, Rule>> for Imm {
    fn from(value: pest::iterators::Pair<'_, Rule>) -> Self {
        if value.as_rule() == Rule::label {
            Self::Label(value.as_str().to_string())
        } else {
            let s = value.as_str();
            let s = s.strip_prefix('$').unwrap_or(s);
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
pub type Inst = isa::Inst<Imm>;

impl Inst {
    fn desymbol(&self, sym: &SymbolMap) -> isa::Inst<u64> {
        use isa::Inst::*;
        match self {
            HALT => HALT,
            NOP => NOP,
            CMOVX(cond, ra, rb) => CMOVX(*cond, *ra, *rb),
            IRMOVQ(rb, v) => IRMOVQ(*rb, v.desymbol(sym)),
            RMMOVQ(ra, addr) => RMMOVQ(*ra, *addr),
            MRMOVQ(addr, ra) => MRMOVQ(*addr, *ra),
            OPQ(op, ra, rb) => OPQ(*op, *ra, *rb),
            JX(cond, v) => JX(*cond, v.desymbol(sym)),
            CALL(v) => CALL(v.desymbol(sym)),
            RET => RET,
            PUSHQ(ra) => PUSHQ(*ra),
            POPQ(ra) => POPQ(*ra),
            IOPQ(_, _) => todo!(),
        }
    }
}

/// (a, b) => (a as u8 << 4 | b as u8)
macro_rules! h2 {
    ($a:expr, $b:expr) => {
        ($a as u8) << 4 | ($b as u8)
    };
}

impl SourceInfo {
    pub fn write_object(&self, obj: &mut Object) {
        if let Some(addr) = self.addr {
            let addr = addr as usize;
            if let Some(inst) = &self.inst {
                match inst.desymbol(&obj.symbols) {
                    isa::Inst::HALT => obj.binary[addr] = h2!(inst.icode(), 0),
                    isa::Inst::NOP => obj.binary[addr] = h2!(inst.icode(), 0),
                    isa::Inst::CMOVX(c, ra, rb) => {
                        obj.binary[addr] = h2!(inst.icode(), c as u8);
                        obj.binary[addr + 1] = h2!(ra, rb);
                    }
                    isa::Inst::IRMOVQ(rb, v) => {
                        obj.binary[addr] = h2!(inst.icode(), 0);
                        obj.binary[addr + 1] = h2!(Reg::RNONE, rb);
                        obj.write_num_data(addr + 2, 8, v);
                    }
                    isa::Inst::RMMOVQ(ra, Addr(dis, rb)) => {
                        obj.binary[addr] = h2!(inst.icode(), 0);
                        obj.binary[addr + 1] = h2!(ra, rb);
                        let data = dis.unwrap_or(0);
                        obj.write_num_data(addr + 2, 8, data);
                    }
                    isa::Inst::MRMOVQ(Addr(dis, rb), ra) => {
                        obj.binary[addr] = h2!(inst.icode(), 0);
                        obj.binary[addr + 1] = h2!(ra, rb);
                        let data = dis.unwrap_or(0);
                        obj.write_num_data(addr + 2, 8, data);
                    }
                    isa::Inst::OPQ(op, ra, rb) => {
                        obj.binary[addr] = h2!(inst.icode(), op as u8);
                        obj.binary[addr + 1] = h2!(ra, rb);
                    }
                    isa::Inst::JX(c, dest) => {
                        obj.binary[addr] = h2!(inst.icode(), c as u8);
                        obj.write_num_data(addr + 1, 8, dest);
                    }
                    isa::Inst::CALL(dest) => {
                        obj.binary[addr] = h2!(inst.icode(), 0);
                        obj.write_num_data(addr + 1, 8, dest);
                    }
                    isa::Inst::RET => obj.binary[addr] = h2!(inst.icode(), 0),
                    isa::Inst::PUSHQ(ra) => {
                        obj.binary[addr] = h2!(inst.icode(), 0);
                        obj.binary[addr + 1] = h2!(ra, Reg::RNONE);
                    }
                    isa::Inst::POPQ(ra) => {
                        obj.binary[addr] = h2!(inst.icode(), 0);
                        obj.binary[addr + 1] = h2!(ra, Reg::RNONE);
                    }
                    isa::Inst::IOPQ(_, _) => todo!(),
                }
            }
            if let Some((sz, data)) = &self.data {
                let data = data.desymbol(&obj.symbols);
                obj.write_num_data(addr, *sz, data);
            }
        }
    }
}

#[derive(Debug)]
pub struct SourceInfo {
    pub addr: Option<u64>,
    pub inst: Option<Inst>,
    pub label: Option<String>,
    // width and data
    pub data: Option<(u8, Imm)>,
    pub src: String,
}

/// object file
///
/// while y86 language support 64-bit address, we only consider address < 0x10000.
pub struct Object {
    pub binary: [u8; BIN_SIZE],
    /// basically labels
    pub symbols: SymbolMap,
}

impl Object {
    fn write_num_data(&mut self, addr: usize, sz: u8, data: u64) {
        for i in 0..sz as usize {
            let byte = (data >> (i * 8) & 0xff) as u8;
            self.binary[addr + i] = byte // little endian
        }
    }
}

/// object file
///
/// while y86 language support 64-bit address, we only consider address < 0x10000.
#[derive(Default)]
pub struct ObjectExt {
    pub obj: Object,
    /// annotate each line with its address
    pub source: Vec<SourceInfo>,
}

impl Default for Object {
    fn default() -> Self {
        Self {
            binary: [0; BIN_SIZE],
            symbols: Default::default(),
        }
    }
}

impl Display for ObjectExt {
    /// display yo format
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for src in &self.source {
            if let Some(addr) = src.addr {
                let addr = addr as usize;
                write!(f, "{:#06x}: ", addr)?;
                if let Some(inst) = &src.inst {
                    for i in 0..inst.len() {
                        write!(f, "{:02x}", self.obj.binary[i + addr])?;
                    }
                    write!(f, "{: <1$}", "", 21 - inst.len() * 2)?
                } else if let Some((sz, _)) = &src.data {
                    for i in 0..*sz as usize {
                        write!(f, "{:02x}", self.obj.binary[i + addr])?;
                    }
                    write!(f, "{: <1$}", "", 21 - *sz as usize * 2)?
                } else {
                    write!(f, "{: <21}", "")?
                }
            } else {
                write!(f, "{: <29}", "")?
            }
            writeln!(f, "| {}", src.src)?
        }
        Ok(())
    }
}

pub fn mem_diff(left: &[u8; BIN_SIZE], right: &[u8; BIN_SIZE]) {
    for i in 0..BIN_SIZE >> 3 {
        if get_u64(&left[i << 3..]) != get_u64(&right[i << 3..]) {
            eprint!("{:#06x}: ", i << 3,);
            for byte in left[i << 3..].iter().take(8) {
                eprint!("{:02x}", *byte)
            }
            eprint!(" -> ");
            for byte in right[i << 3..].iter().take(8) {
                eprint!("{:02x}", *byte)
            }
            eprintln!()
        }
    }
}

pub fn mem_print(bin: &[u8; BIN_SIZE]) {
    let mut max_i = 0;
    for i in 0..BIN_SIZE >> 3 {
        if get_u64(&bin[i << 3..]) != 0 {
            max_i = i;
        }
    }
    for i in 0..=max_i {
        eprint!("{:#06x}: ", i << 3);
        for byte in bin[i << 3..].iter().take(8) {
            eprint!("{:02x}", *byte)
        }
        eprintln!()
    }
}
