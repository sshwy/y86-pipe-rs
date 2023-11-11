//! This module provides parsing utilities for the y86 assembly.
use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "src/grammer.pest"] // relative to src
pub struct Y86AsmParser;

pub fn parse(src: &str) -> pest::iterators::Pairs<'_, Rule> {
    let lines = Y86AsmParser::parse(Rule::main, src)
        .unwrap()
        .next()
        .unwrap()
        .into_inner();

    lines
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
