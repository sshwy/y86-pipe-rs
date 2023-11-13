//! This module provides parsing utilities for the y86 assembly.
use anyhow::{Context, Result};

use pest::Parser;
use pest_derive::Parser;

use crate::{
    isa::{Addr, CondFn, OpFn, Reg},
    object::{self, Object, ObjectExt, SourceInfo},
};

#[derive(Parser)]
#[grammar = "src/grammer.pest"] // relative to src
pub struct Y86AsmParser;

pub fn parse(src: &str) -> Result<pest::iterators::Pairs<'_, Rule>> {
    Ok(Y86AsmParser::parse(Rule::main, src)
        .context("fail to parse ys file")?
        .next()
        .unwrap()
        .into_inner())
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
    let lines = parse(src).context("fail to assemble ys file")?;
    let mut cur_addr = u64::default();

    for line in lines {
        let src = line.as_str().to_string();
        let mut line = line.into_inner();
        let mut src_info = SourceInfo {
            addr: None,
            inst: None,
            label: None,
            data: None,
            src,
        };
        if let Some(pair) = line.next() {
            verbo!(&pair);
            src_info.addr = Some(cur_addr);
            let pair2 = pair.clone();
            let mut it = pair.into_inner();
            match pair2.as_rule() {
                Rule::label => src_info.label = Some(pair2.as_str().to_string()),
                Rule::i_single => {
                    src_info.inst = Some(match pair2.as_str() {
                        "halt" => object::Inst::HALT,
                        "nop" => object::Inst::NOP,
                        "ret" => object::Inst::RET,
                        _ => panic!("invalid instruction"),
                    });
                    cur_addr += 1
                }
                Rule::i_cmovq => {
                    let cond_fn = CondFn::from(it.next().unwrap().as_str());
                    let reg_a = Reg::from(it.next().unwrap());
                    let reg_b = Reg::from(it.next().unwrap());
                    src_info.inst = Some(object::Inst::CMOVX(cond_fn, reg_a, reg_b));
                    cur_addr += 2
                }
                Rule::i_mrmovq => {
                    let addr = Addr::from(it.next().unwrap());
                    let reg = Reg::from(it.next().unwrap());
                    src_info.inst = Some(object::Inst::MRMOVQ(addr, reg));
                    cur_addr += 10
                }
                Rule::i_rmmovq => {
                    let reg = Reg::from(it.next().unwrap());
                    let addr = Addr::from(it.next().unwrap());
                    src_info.inst = Some(object::Inst::RMMOVQ(reg, addr));
                    cur_addr += 10
                }
                Rule::i_irmovq => {
                    let imm = object::Imm::from(it.next().unwrap());
                    let reg = Reg::from(it.next().unwrap());
                    src_info.inst = Some(object::Inst::IRMOVQ(reg, imm));
                    cur_addr += 10
                }
                Rule::i_opq => {
                    let reg_a = Reg::from(it.next().unwrap());
                    let reg_b = Reg::from(it.next().unwrap());
                    let op_fn = OpFn::from(pair2.as_str());
                    src_info.inst = Some(object::Inst::OPQ(op_fn, reg_a, reg_b));
                    cur_addr += 2
                }
                Rule::i_iopq => todo!(),
                Rule::i_jx => {
                    let cond_fn = CondFn::from(it.next().unwrap().as_str());
                    let imm = object::Imm::from(it.next().unwrap());
                    src_info.inst = Some(object::Inst::JX(cond_fn, imm));
                    cur_addr += 9
                }
                Rule::i_call => {
                    let imm = object::Imm::from(it.next().unwrap());
                    src_info.inst = Some(object::Inst::CALL(imm));
                    cur_addr += 9
                }
                Rule::i_pushq => {
                    let reg = Reg::from(it.next().unwrap());
                    src_info.inst = Some(object::Inst::PUSHQ(reg));
                    cur_addr += 2
                }
                Rule::i_popq => {
                    let reg = Reg::from(it.next().unwrap());
                    src_info.inst = Some(object::Inst::POPQ(reg));
                    cur_addr += 2
                }
                Rule::d_pos => {
                    let s = it.next().unwrap().as_str();
                    let num = if let Ok(r) = s.parse() {
                        r
                    } else {
                        u64::from_str_radix(&s[2..], 16).unwrap()
                    };

                    cur_addr = num;
                    src_info.addr = Some(cur_addr) // override
                }
                Rule::d_data => {
                    let imm = object::Imm::from(it.next().unwrap());
                    if pair2.as_str().starts_with(".quad") {
                        src_info.data = Some((8, imm));
                        cur_addr += 8;
                    } else {
                        todo!()
                    }
                }
                Rule::d_align => {
                    let s = it.next().unwrap().as_str();
                    let num = if let Ok(r) = s.parse() {
                        r
                    } else {
                        i64::from_str_radix(&s[2..], 16).unwrap()
                    };
                    assert!(num & (-num) == num); // 2^k
                    let num = num as u64;
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
        source: src_infos,
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
