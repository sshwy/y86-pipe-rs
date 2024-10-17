//! This file provides binary representation of y86 instructions

use std::{collections::BTreeMap, fmt::Display};

use crate::{
    asm::{self, Reg},
    framework::MEM_SIZE,
};

/// Maximum size of the assembled binary.
pub const BIN_SIZE: usize = MEM_SIZE;

type SymbolMap = BTreeMap<String, u64>;

impl asm::Imm {
    fn desymbol(&self, sym: &SymbolMap) -> u64 {
        match self {
            asm::Imm::Num(n) => *n as u64,
            asm::Imm::Label(label) => sym[label],
        }
    }
}
impl asm::Inst<asm::Imm> {
    pub fn desymbol(&self, sym: &SymbolMap) -> asm::Inst<u64> {
        use asm::Inst::*;
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
            IOPQ(op, imm, reg) => IOPQ(*op, imm.desymbol(sym), *reg),
        }
    }
}

/// (a, b) => (a as u8 << 4 | b as u8)
macro_rules! h2 {
    ($a:expr, $b:expr) => {
        ($a as u8) << 4 | ($b as u8)
    };
}

impl LineInfo {
    pub fn write_object(&self, obj: &mut Object) {
        if let Some(addr) = self.addr {
            let addr = addr as usize;
            if let Some(inst) = &self.inst {
                match inst.desymbol(&obj.symbols) {
                    asm::Inst::HALT => obj.binary[addr] = h2!(inst.icode(), 0),
                    asm::Inst::NOP => obj.binary[addr] = h2!(inst.icode(), 0),
                    asm::Inst::CMOVX(c, ra, rb) => {
                        obj.binary[addr] = h2!(inst.icode(), c as u8);
                        obj.binary[addr + 1] = h2!(ra, rb);
                    }
                    asm::Inst::IRMOVQ(rb, v) => {
                        obj.binary[addr] = h2!(inst.icode(), 0);
                        obj.binary[addr + 1] = h2!(Reg::RNONE, rb);
                        obj.write_num_data(addr + 2, 8, v);
                    }
                    asm::Inst::RMMOVQ(ra, asm::Addr(dis, rb)) => {
                        obj.binary[addr] = h2!(inst.icode(), 0);
                        obj.binary[addr + 1] = h2!(ra, rb);
                        let data = dis.unwrap_or(0);
                        obj.write_num_data(addr + 2, 8, data);
                    }
                    asm::Inst::MRMOVQ(asm::Addr(dis, rb), ra) => {
                        obj.binary[addr] = h2!(inst.icode(), 0);
                        obj.binary[addr + 1] = h2!(ra, rb);
                        let data = dis.unwrap_or(0);
                        obj.write_num_data(addr + 2, 8, data);
                    }
                    asm::Inst::OPQ(op, ra, rb) => {
                        obj.binary[addr] = h2!(inst.icode(), op as u8);
                        obj.binary[addr + 1] = h2!(ra, rb);
                    }
                    asm::Inst::JX(c, dest) => {
                        obj.binary[addr] = h2!(inst.icode(), c as u8);
                        obj.write_num_data(addr + 1, 8, dest);
                    }
                    asm::Inst::CALL(dest) => {
                        obj.binary[addr] = h2!(inst.icode(), 0);
                        obj.write_num_data(addr + 1, 8, dest);
                    }
                    asm::Inst::RET => obj.binary[addr] = h2!(inst.icode(), 0),
                    asm::Inst::PUSHQ(ra) => {
                        obj.binary[addr] = h2!(inst.icode(), 0);
                        obj.binary[addr + 1] = h2!(ra, Reg::RNONE);
                    }
                    asm::Inst::POPQ(ra) => {
                        obj.binary[addr] = h2!(inst.icode(), 0);
                        obj.binary[addr + 1] = h2!(ra, Reg::RNONE);
                    }
                    asm::Inst::IOPQ(op, v, rb) => {
                        obj.binary[addr] = h2!(inst.icode(), op as u8);
                        obj.binary[addr + 1] = h2!(Reg::RNONE, rb);
                        obj.write_num_data(addr + 2, 8, v);
                    }
                }
            }
            if let Some((sz, data)) = &self.data {
                let data = data.desymbol(&obj.symbols);
                obj.write_num_data(addr, *sz, data);
            }
        }
    }
}

/// A source info is one line of the .yo file.
#[derive(Debug, Clone)]
pub struct LineInfo {
    pub addr: Option<u64>,
    pub inst: Option<asm::Inst<asm::Imm>>,
    pub label: Option<String>,
    // width and data
    pub data: Option<(u8, asm::Imm)>,
    pub src: String,
}

/// object file
///
/// while y86 language support 64-bit address, we only consider address <
/// 0x10000.
pub struct Object {
    binary: [u8; BIN_SIZE],
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

    /// Initialize memory from assembled binary.
    pub fn init_mem(&self) -> [u8; MEM_SIZE] {
        let mut mem = [0; MEM_SIZE];
        mem[..self.binary.len()].copy_from_slice(&self.binary);
        mem
    }
}

#[derive(Debug, Default)]
pub struct SourceInfo(Vec<LineInfo>);

impl From<Vec<LineInfo>> for SourceInfo {
    fn from(v: Vec<LineInfo>) -> Self {
        Self(v)
    }
}

impl SourceInfo {
    /// Get the line info by line number (start from 1)
    pub fn get_line(&self, line: i64) -> Option<&LineInfo> {
        assert!(line > 0);
        self.0.get(line as usize - 1)
    }

    pub fn get_line_number_by_addr(&self, addr: u64) -> Option<i64> {
        self.0
            .iter()
            .position(|x| x.addr == Some(addr))
            .map(|x| x as i64 + 1)
    }
}

/// object file with source info.
///
/// To render the yo file, use `format!("{}", obj)`.
#[derive(Default)]
pub struct ObjectExt {
    pub obj: Object,
    /// annotate each line with its address
    pub source: SourceInfo,
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
        for src in &self.source.0 {
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
