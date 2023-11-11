use bin::{Object, SourceInfo};

mod bin;
mod parse;

pub fn assemble(src: &str) {
    let mut obj = Object::default();
    let lines = parse::parse(src);
    let mut cur_addr = u16::default();

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
            dbg!(&pair);
            src_info.addr = Some(cur_addr);
            let pair2 = pair.clone();
            let mut it = pair.into_inner();
            match pair2.as_rule() {
                parse::Rule::label => src_info.label = Some(pair2.as_str().to_string()),
                parse::Rule::i_single => {
                    src_info.inst = Some(match pair2.as_str() {
                        "halt" => bin::Inst::HALT,
                        "nop" => bin::Inst::NOP,
                        "ret" => bin::Inst::RET,
                        _ => panic!("invalid instruction"),
                    });
                    cur_addr += 1
                }
                parse::Rule::i_cmovq => {
                    let cond_fn = it.next().unwrap();
                    let reg_a = bin::Reg::from(it.next().unwrap().as_str());
                    let reg_b = bin::Reg::from(it.next().unwrap().as_str());
                    let cond_fn = if cond_fn.as_rule() == parse::Rule::rrmovq {
                        None
                    } else {
                        Some(bin::CondFn::from(cond_fn.as_str()))
                    };
                    src_info.inst = Some(bin::Inst::CMOVX(cond_fn, reg_a, reg_b));
                    cur_addr += 2
                }
                parse::Rule::i_mrmovq => {
                    let addr = bin::Addr::from(it.next().unwrap());
                    let reg = bin::Reg::from(it.next().unwrap().as_str());
                    src_info.inst = Some(bin::Inst::MRMOVQ(addr, reg));
                    cur_addr += 10
                }
                parse::Rule::i_rmmovq => {
                    let reg = bin::Reg::from(it.next().unwrap().as_str());
                    let addr = bin::Addr::from(it.next().unwrap());
                    src_info.inst = Some(bin::Inst::RMMOVQ(reg, addr));
                    cur_addr += 10
                }
                parse::Rule::i_irmovq => {
                    let imm = bin::Imm::from(it.next().unwrap());
                    let reg = bin::Reg::from(it.next().unwrap().as_str());
                    src_info.inst = Some(bin::Inst::IRMOVQ(reg, imm));
                    cur_addr += 10
                }
                parse::Rule::i_opq => {
                    let reg_a = bin::Reg::from(it.next().unwrap().as_str());
                    let reg_b = bin::Reg::from(it.next().unwrap().as_str());
                    let op_fn = bin::OpFn::from(pair2.as_str());
                    src_info.inst = Some(bin::Inst::OPQ(op_fn, reg_a, reg_b));
                    cur_addr += 2
                }
                parse::Rule::i_iopq => todo!(),
                parse::Rule::i_jx => {
                    let cond_fn = it.next().unwrap();
                    let imm = bin::Imm::from(it.next().unwrap());
                    let cond_fn = if cond_fn.as_rule() == parse::Rule::mp_suf {
                        None
                    } else {
                        Some(bin::CondFn::from(cond_fn.as_str()))
                    };
                    src_info.inst = Some(bin::Inst::JX(cond_fn, imm));
                    cur_addr += 9
                }
                parse::Rule::i_call => {
                    let imm = bin::Imm::from(it.next().unwrap());
                    src_info.inst = Some(bin::Inst::CALL(imm));
                    cur_addr += 9
                }
                parse::Rule::i_pushq => {
                    let reg = bin::Reg::from(it.next().unwrap().as_str());
                    src_info.inst = Some(bin::Inst::PUSHQ(reg));
                    cur_addr += 2
                }
                parse::Rule::i_popq => {
                    let reg = bin::Reg::from(it.next().unwrap().as_str());
                    src_info.inst = Some(bin::Inst::POPQ(reg));
                    cur_addr += 2
                }
                parse::Rule::d_pos => {
                    let s = it.next().unwrap().as_str();
                    let num = if let Ok(r) = s.parse() {
                        r
                    } else {
                        u16::from_str_radix(&s[2..], 16).unwrap()
                    };

                    cur_addr = num;
                    src_info.addr = Some(cur_addr) // override
                }
                parse::Rule::d_data => {
                    let s = it.next().unwrap().as_str();
                    let num = if let Ok(r) = s.parse() {
                        r
                    } else {
                        i64::from_str_radix(&s[2..], 16).unwrap()
                    };
                    if pair2.as_str().starts_with(".quad") {
                        src_info.data = Some((8, num as u64));
                        cur_addr += 8;
                    } else {
                        todo!()
                    }
                }
                parse::Rule::d_align => {
                    let s = it.next().unwrap().as_str();
                    let num = if let Ok(r) = s.parse() {
                        r
                    } else {
                        i64::from_str_radix(&s[2..], 16).unwrap()
                    };
                    assert!(num & (-num) == num); // 2^k
                    let num = num as u16;
                    if cur_addr % num > 0 {
                        cur_addr = cur_addr / num * num + num // ceil
                    }
                    src_info.addr = Some(cur_addr) // override
                }
                _ => unimplemented!(),
            }
        }
        dbg!(&src_info);
        obj.source.push(src_info);
    }
    for info in &obj.source {
        if let Some(label) = &info.label {
            obj.symbols.insert(label.clone(), info.addr.unwrap());
        }
    }
    dbg!(&obj.symbols);
}

#[cfg(test)]
mod tests {
    use crate::assemble;

    #[test]
    fn test_assemble() {
        assemble(crate::parse::tests::RSUM_YS)
    }
}
