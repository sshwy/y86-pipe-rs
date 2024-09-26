//! This module provides parsing utilities for the y86 assembly.
use anyhow::{bail, Context, Result};
use pest::Parser;
use pest_derive::Parser;

use crate::{
    isa::{self, reg_code},
    object::{LineInfo, Object, ObjectExt},
    utils, SourceInfo,
};

#[derive(Parser)]
#[grammar = "src/asm/grammer.pest"] // relative to src
pub struct Y86AsmParser;

pub fn parse(src: &str) -> Result<pest::iterators::Pair<'_, Rule>> {
    Ok(Y86AsmParser::parse(Rule::main, src)
        .context("fail to parse ys file")?
        .next()
        .unwrap())
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

impl TryFrom<&str> for Reg {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        Ok(match value {
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
            _ => return Err(anyhow::anyhow!("invalid register identifier")),
        })
    }
}

impl From<pest::iterators::Pair<'_, Rule>> for Reg {
    fn from(value: pest::iterators::Pair<'_, Rule>) -> Self {
        value.as_str().try_into().unwrap()
    }
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

impl OpFn {
    fn from_instname(value: &str) -> Self {
        if value.contains("andq") {
            Self::AND
        } else if value.contains("addq") {
            Self::ADD
        } else if value.contains("subq") {
            Self::SUB
        } else if value.contains("xorq") {
            Self::XOR
        } else {
            panic!("invalid")
        }
    }
}

/// Address mode expression with optional displacement
#[derive(Debug, Clone, Copy)]
pub struct Addr(pub Option<u64>, pub Reg);

impl From<ParseInput<'_>> for Addr {
    fn from(value: ParseInput<'_>) -> Self {
        let mut it = value.into_iter();
        let num_or_reg = it.next().unwrap();
        if num_or_reg.as_rule() == Rule::reg {
            // no displacement
            let reg = Reg::from(num_or_reg.pair);
            Self(None, reg)
        } else {
            let s = num_or_reg.as_str();
            let num = utils::parse_literal(s).unwrap() as i64;
            let reg = it.next_reg();
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

impl From<ParseInput<'_>> for Imm {
    fn from(value: ParseInput<'_>) -> Self {
        if value.as_rule() == Rule::label {
            Self::Label(value.as_str().to_string())
        } else {
            let s = value.as_str();
            let s = s.strip_prefix('$').unwrap_or(s);
            let num = utils::parse_literal(s).unwrap() as i64;
            Self::Num(num)
        }
    }
}

/// Y86 instruction Set.
///
/// During assembling, the type of immediate (`ImmType`) can change.
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
    IOPQ(OpFn, ImmType, Reg),
}

impl<ImmType: Clone> Inst<ImmType> {
    pub fn len(&self) -> usize {
        use Inst::*;
        match self {
            HALT | RET | NOP => 1,
            OPQ(_, _, _) | CMOVX(_, _, _) | PUSHQ(_) | POPQ(_) => 2,
            JX(_, _) | CALL(_) => 9,
            IRMOVQ(_, _) | RMMOVQ(_, _) | MRMOVQ(_, _) => 10,
            // extended instructions
            IOPQ(_, _, _) => 10,
        }
    }

    pub fn icode(&self) -> u8 {
        use isa::inst_code::*;
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
            // extended instructions
            Inst::IOPQ(_, _, _) => IOPQ,
        }
    }
}

#[derive(Default)]
pub struct AssembleOption {
    verbose: bool,
}

impl AssembleOption {
    pub fn set_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }
}

#[derive(Debug, Clone)]
struct ParseInput<'a> {
    pair: pest::iterators::Pair<'a, Rule>,
}

impl<'a> IntoIterator for ParseInput<'a> {
    type IntoIter = ParseInputs<'a>;
    type Item = ParseInput<'a>;

    fn into_iter(self) -> Self::IntoIter {
        ParseInputs {
            pairs: self.pair.into_inner(),
        }
    }
}

impl<'a> ParseInput<'a> {}

impl<'a> std::ops::Deref for ParseInput<'a> {
    type Target = pest::iterators::Pair<'a, Rule>;

    fn deref(&self) -> &Self::Target {
        &self.pair
    }
}

struct ParseInputs<'a> {
    pairs: pest::iterators::Pairs<'a, Rule>,
}

impl<'a> Iterator for ParseInputs<'a> {
    type Item = ParseInput<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.pairs.next().map(|pair| ParseInput { pair })
    }
}

impl<'a> ParseInputs<'a> {
    /// Parse next token as string
    fn next_str(&mut self) -> &str {
        self.next().unwrap().as_str()
    }

    /// Parse next token as register
    fn next_reg(&mut self) -> Reg {
        Reg::from(self.next().unwrap().pair)
    }

    /// Parse next token as address
    fn next_addr(&mut self) -> Addr {
        Addr::from(self.next().unwrap())
    }
}

/// transform assembly code to binary object code
pub fn assemble(src: &str, option: AssembleOption) -> Result<ObjectExt> {
    macro_rules! verbo {
        ($e:expr) => {
            if option.verbose {
                dbg!($e);
            }
        };
    }
    let mut src_infos = Vec::default();
    let lines = ParseInput {
        pair: parse(src).context("fail to assemble ys file")?,
    };
    let mut cur_addr = u64::default();

    for line in lines {
        let src = line.pair.as_str().to_string();
        let mut line = line.into_iter();
        let mut src_info = LineInfo {
            addr: None,
            inst: None,
            label: None,
            data: None,
            src,
        };
        if let Some(pair) = line.pairs.peek() {
            // set addr for instruction or label
            src_info.addr = Some(cur_addr);

            if let Rule::label = pair.as_rule() {
                src_info.label = Some(pair.as_str().to_string());
                // consume this label
                line.next();
            }
        }
        // if this line has instruction
        if let Some(pair) = line.next() {
            verbo!(&pair);
            let tok2 = pair.clone();
            let mut it = pair.into_iter();
            match tok2.as_rule() {
                Rule::label => bail!("unexpected label"),
                Rule::i_single => {
                    src_info.inst = Some(match tok2.as_str() {
                        "halt" => Inst::HALT,
                        "nop" => Inst::NOP,
                        "ret" => Inst::RET,
                        _ => panic!("invalid instruction"),
                    });
                    cur_addr += 1
                }
                Rule::i_cmovq => {
                    let cond_fn = CondFn::from(it.next_str());
                    let reg_a = it.next_reg();
                    let reg_b = it.next_reg();
                    src_info.inst = Some(Inst::CMOVX(cond_fn, reg_a, reg_b));
                    cur_addr += 2
                }
                Rule::i_mrmovq => {
                    let addr = it.next_addr();
                    let reg = it.next_reg();
                    src_info.inst = Some(Inst::MRMOVQ(addr, reg));
                    cur_addr += 10
                }
                Rule::i_rmmovq => {
                    let reg = it.next_reg();
                    let addr = it.next_addr();
                    src_info.inst = Some(Inst::RMMOVQ(reg, addr));
                    cur_addr += 10
                }
                Rule::i_irmovq => {
                    let imm = Imm::from(it.next().unwrap());
                    let reg = it.next_reg();
                    src_info.inst = Some(Inst::IRMOVQ(reg, imm));
                    cur_addr += 10
                }
                Rule::i_opq => {
                    let reg_a = it.next_reg();
                    let reg_b = it.next_reg();
                    let op_fn = OpFn::from_instname(tok2.as_str());
                    src_info.inst = Some(Inst::OPQ(op_fn, reg_a, reg_b));
                    cur_addr += 2
                }
                Rule::i_iopq => {
                    let imm = Imm::from(it.next().unwrap());
                    let reg = it.next_reg();
                    let op_fn = OpFn::from_instname(tok2.as_str());
                    src_info.inst = Some(Inst::IOPQ(op_fn, imm, reg));
                    cur_addr += 10
                }
                Rule::i_jx => {
                    let cond_fn = CondFn::from(it.next_str());
                    let imm = Imm::from(it.next().unwrap());
                    src_info.inst = Some(Inst::JX(cond_fn, imm));
                    cur_addr += 9
                }
                Rule::i_call => {
                    let imm = Imm::from(it.next().unwrap());
                    src_info.inst = Some(Inst::CALL(imm));
                    cur_addr += 9
                }
                Rule::i_pushq => {
                    let reg = it.next_reg();
                    src_info.inst = Some(Inst::PUSHQ(reg));
                    cur_addr += 2
                }
                Rule::i_popq => {
                    let reg = it.next_reg();
                    src_info.inst = Some(Inst::POPQ(reg));
                    cur_addr += 2
                }
                Rule::d_pos => {
                    let s = it.next_str();
                    let num = utils::parse_literal(s).unwrap();
                    cur_addr = num;
                    src_info.addr = Some(cur_addr) // override
                }
                Rule::d_data => {
                    let imm = Imm::from(it.next().unwrap());
                    if tok2.as_str().starts_with(".quad") {
                        src_info.data = Some((8, imm));
                        cur_addr += 8;
                    } else if tok2.as_str().starts_with(".long") {
                        src_info.data = Some((4, imm));
                        cur_addr += 4;
                    } else if tok2.as_str().starts_with(".word") {
                        src_info.data = Some((2, imm));
                        cur_addr += 2;
                    } else if tok2.as_str().starts_with(".byte") {
                        src_info.data = Some((1, imm));
                        cur_addr += 1;
                    } else {
                        bail!("invalid data directive: {}", tok2.as_str())
                    }
                }
                Rule::d_align => {
                    let s = it.next_str();
                    let num = utils::parse_literal(s).unwrap();
                    anyhow::ensure!(num.count_ones() == 1, "invalid align number");
                    if cur_addr % num > 0 {
                        cur_addr = cur_addr / num * num + num // ceil
                    }
                    src_info.addr = Some(cur_addr) // override
                }
                _ => unimplemented!(),
            }
        }
        verbo!(&src_info);
        src_infos.push(src_info);
    }
    let mut obj = Object::default();
    for info in &src_infos {
        if let Some(label) = &info.label {
            obj.symbols.insert(label.clone(), info.addr.unwrap());
        }
    }
    verbo!(&obj.symbols);

    for it in &src_infos {
        it.write_object(&mut obj)
    }

    Ok(ObjectExt {
        obj,
        source: SourceInfo::from(src_infos),
    })
}

#[cfg(test)]
pub mod tests {
    use pest::Parser;

    use super::{Rule, Y86AsmParser};

    pub const RSUM_YS: &str = r#"
# Weiyao Huang 2200012952
    .pos 0 # start position FIXME: why does memory change
    irmovq stack, %rsp
    irmovq ele1, %rdi
    call sum_list
    halt

sum_list: # %rdi = ls
    pushq %rbx
    irmovq $0, %rax

    rrmovq %rdi, %rbx
    andq %rdi, %rbx
    je sum_list_ret
    
    mrmovq (%rdi), %rbx
    addq %rbx, %rax
    mrmovq 8(%rdi), %rdi

    pushq %rax
    call sum_list
    popq %rbx
    addq %rbx, %rax

    # jmp sum_list_while_cond
sum_list_ret:

    popq %rbx
    ret

    .align 8
ele1:
    .quad 0x00a
    .quad ele2
ele2:
    .quad 0x0b0
    .quad ele3
ele3:
    .quad 0xc00
    .quad 0

    .pos 0x200
stack: # start of stack
"#;

    #[test]
    fn test_parser() {
        let lines = Y86AsmParser::parse(Rule::main, RSUM_YS)
            .unwrap()
            .next()
            .unwrap()
            .into_inner();
        // iterate all lines
        for line in lines.filter(|l| l.as_rule() == Rule::line) {
            dbg!(line);
        }
    }
}
