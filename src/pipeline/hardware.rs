//! hardware behavior definition

use std::cell::RefCell;
use std::rc::Rc;

use super::Stat;
use crate::isa::cond_fn::*;
use crate::isa::inst_code::NOP;
use crate::isa::op_code::*;
use crate::isa::reg_code;
use crate::isa::reg_code::*;
use crate::{
    define_devices,
    isa::BIN_SIZE,
    utils::{get_u64, put_u64},
};

define_devices! {
    // stage registers and default values for bubble status

    /// Fetch stage registers.
    /// note that it's not possible to bubble (see hcl)
    Fstage f {
        .input(stall: bool, bubble: bool)
        .pass(pred_pc: u64 = 0)
    } {
    }
    Dstage d {
        .input(stall: bool, bubble: bool)
        .pass(stat: Stat = Stat::Bub, icode: u8 = NOP, ifun: u8 = 0,
            ra: u8 = RNONE, rb: u8 = RNONE, valc: u64 = 0, valp: u64 = 0)
    }
    Estage e {
        .input(stall: bool, bubble: bool)
        .pass(stat: Stat = Stat::Bub, icode: u8 = NOP, ifun: u8 = 0,
            vala: u64 = 0, valb: u64 = 0, valc: u64 = 0, dste: u8 = RNONE,
            dstm: u8 = RNONE, srca: u8 = RNONE, srcb: u8 = RNONE)
    }
    Mstage m {
        .input(stall: bool, bubble: bool)
        .pass(stat: Stat = Stat::Bub, icode: u8 = NOP, cnd: bool = false,
            vale: u64 = 0, vala: u64 = 0, dste: u8 = RNONE, dstm: u8 = RNONE)
    }
    Wstage w {
        .input(stall: bool, bubble: bool)
        .pass(stat: Stat = Stat::Bub, icode: u8 = NOP, vale: u64 = 0,
            valm: u64 = 0, dste: u8 = RNONE, dstm: u8 = RNONE)
    }

    InstructionMemory imem { // with split
        .input(pc: u64)
        .output(error: bool, icode: u8, ifun: u8, align: [u8; 9])
        binary: Rc<RefCell<[u8; BIN_SIZE]>>
    } {
        let binary: &[u8; BIN_SIZE] = &binary.borrow();
        if pc + 10 > BIN_SIZE as u64 {
            *error = true;
        } else {
            let pc = pc as usize;
            let icode_ifun = binary[pc];
            *icode = icode_ifun >> 4;
            *ifun = icode_ifun & 0xf;
            *align = binary[pc+1..pc+10].try_into().unwrap();
        }
    }

    Align ialign {
        .input(need_regids: bool, align: [u8; 9])
        .output(ra: u8, rb: u8, valc: u64)
    } {
        let ra_rb = align[0];
        let rest = if need_regids {
            *ra = ra_rb >> 4;
            *rb = ra_rb & 0xf;
            &align[1..9]
        } else {
            *ra = RNONE;
            *rb = RNONE;
            &align[0..8]
        };
        *valc = get_u64(rest)
    }

    PCIncrement pc_inc {
        .input(need_valc: bool, need_regids: bool, old_pc: u64)
        .output(new_pc: u64)
    } {
        let mut x = old_pc + 1;
        if need_regids { x += 1; }
        if need_valc { x += 8; }
        *new_pc = x;
    }

    RegisterFile reg_file {
        .input(srca: u8, srcb: u8, dste: u8, dstm: u8, vale: u64, valm: u64)
        .output(vala: u64, valb: u64)
        state: [u64; 16]
    } {
        if srca != RNONE {
            *vala = state[srca as usize];
        }
        if srcb != RNONE {
            *valb = state[srcb as usize];
        }
        if dste != RNONE {
            eprintln!("write back fron e: dste = {}, vale = {:#x}", reg_code::name_of(dste), vale);
            state[dste as usize] = vale;
        }
        if dstm != RNONE {
            eprintln!("write back fron m: dstm = {}, valm = {:#x}", reg_code::name_of(dstm), valm);
            state[dstm as usize] = valm;
        }
    }

    ArithmetcLogicUnit alu {
        .input(a: u64, b: u64, fun: u8)
        .output(e: u64)
    } {
        eprintln!("alu: fun = {}", fun);
        *e = match fun {
            ADD => b.wrapping_add(a),
            SUB => b.wrapping_sub(a),
            AND => b & a,
            XOR => b ^ a,
            _ => 0,
        };
        eprintln!("alu: a = {:#x}, b = {:#x}, e = {:#x}", a, b, e);
    }

    ConditionCode cc {
        .input(set_cc: bool, a: u64, b: u64, e: u64, opfun: u8)
        .output(sf: bool, of: bool, zf: bool)
        s_sf: bool,
        s_of: bool,
        s_zf: bool,
    } {
        let cur_sf = (e >> 31 & 1) != 0;
        let cur_zf = e == 0;
        let cur_of = match opfun {
            // a, b have the same sign and a, e have different sign
            ADD => (!(a ^ b) & (a ^ e)) >> 31 != 0,
            // (b - a): a, b have different sign and b, e have different sign
            SUB => ((a ^ b) & (b ^ e)) >> 31 != 0,
            _ => false
        };
        if set_cc {
            *s_sf = cur_sf;
            *s_of = cur_of;
            *s_zf = cur_zf;
        }
        *sf = *s_sf;
        *of = *s_of;
        *zf = *s_zf;
        eprintln!("a = {:#x}, b = {:#x}, e = {:#x}, sf = {sf}, of = {of}, zf = {zf}", a, b, e);
    }

    Condition cond {
        .input(condfun: u8, sf: bool, of: bool, zf: bool)
        .output(cnd: bool)
    } {
        *cnd = match condfun {
            YES => true,
            E => zf,
            NE => !zf,
            L => sf ^ of,
            LE => zf || (sf ^ of),
            GE => !(sf ^ of),
            G => !zf && !(sf ^ of),
            _ => false
        }
    }

    DataMemory dmem {
        .input(addr: u64, datain: u64, read: bool, write: bool)
        .output(dataout: u64, error: bool)
        binary: Rc<RefCell<[u8; BIN_SIZE]>>
    } {
        if addr + 8 >= BIN_SIZE as u64 {
            *dataout = 0;
            *error = true;
            return
        }
        *error = false;
        if write {
            eprintln!("write memory: addr = {:#x}, datain = {:#x}", addr, datain);
            let section: &mut [u8] = &mut binary.borrow_mut()[(addr as usize)..];
            put_u64(section, datain);
            *dataout = 0;
        } else if read {
            *dataout = get_u64(&binary.borrow()[(addr as usize)..]);
        }
    }
}

impl Devices {
    pub(crate) fn init(bin: [u8; BIN_SIZE]) -> Self {
        let cell = std::rc::Rc::new(RefCell::new(bin));
        Self {
            f: Fstage {},
            d: Dstage {},
            e: Estage {},
            m: Mstage {},
            w: Wstage {},
            imem: InstructionMemory {
                binary: cell.clone(),
            },
            ialign: Align {},
            pc_inc: PCIncrement {},
            reg_file: RegisterFile { state: [0; 16] },
            alu: ArithmetcLogicUnit {},
            cc: ConditionCode {
                s_sf: false,
                s_of: false,
                s_zf: false,
            },
            cond: Condition {},
            dmem: DataMemory { binary: cell },
        }
    }
    pub(crate) fn mem(&self) -> [u8; BIN_SIZE] {
        *self.dmem.binary.borrow()
    }
    pub(crate) fn print_reg(&self) -> String {
        format!("%rax {rax:#018x} %rbx {rbx:#018x} %rcx {rcx:#018x} %rdx {rdx:#018x}\n%rsi {rsi:#018x} %rdi {rdi:#018x} %rsp {rsp:#018x} %rbp {rbp:#018x}",
            rax = self.reg_file.state[RAX as usize],
            rbx = self.reg_file.state[RBX as usize],
            rcx = self.reg_file.state[RCX as usize],
            rdx = self.reg_file.state[RDX as usize],
            rsi = self.reg_file.state[RSI as usize],
            rdi = self.reg_file.state[RDI as usize],
            rsp = self.reg_file.state[RSP as usize],
            rbp = self.reg_file.state[RBP as usize],
        )
    }
}
