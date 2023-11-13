//! hardware behavior definition

use std::cell::RefCell;

use super::Stat;
use crate::{
    define_devices,
    object::{get_u64, BIN_SIZE},
};

define_devices! {
    // stage registers
    Fstage f { .pass(pred_pc: u64) }
    Dstage d { .pass(stat: Stat, icode: u8, ifun: u8, ra: u8, rb: u8, valc: u64, valp: u64) }
    Estage e { .pass(stat: Stat, icode: u8, ifun: u8, vala: u64, valb: u64, valc: u64,
                dste: u64, dstm: u64, srca: u64, srcb: u64) }
    Mstage m { .pass(stat: Stat, icode: u8, cnd: u8, vale: u64, vala: u64, dste: u64, dstm: u64) }
    Wstage w { .pass(stat: Stat, icode: u8, vale: u64, valm: u64, dste: u64, dstm: u64) }

    InstructionMemory imem { // with split
        .input(pc: u64)
        .output(error: bool, icode: u8, ifun: u8, align: [u8; 9])
        binary: RefCell<[u8; BIN_SIZE]>
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
            &align[0..8]
        };
        *valc = get_u64(rest)
    }

    PCIncrement pc_inc {
        .input(need_valc: bool, need_regids: bool, old_pc: u64)
        .output(new_pc: u64)
    } {
        let mut x = old_pc;
        if need_regids { x += 1; }
        if need_valc { x += 8; }
        *new_pc = x;
    }

    // ArithmetcLogicUnit alu {
    //     .input(a: u64, b: u64, fun: u8)
    //     .output(sf: u8, of: u8, zf: u8, e: u64)
    // } {
    //     let c = match OpFn::from(fun) {
    //         OpFn::ADD => a + b,
    //         OpFn::SUB => a - b,
    //         OpFn::AND => a & b,
    //         OpFn::XOR => a ^ b,
    //     };
    //     *sf = (c >> 31 & 1) as u8;
    //     *of = (((a ^ c) & !(a ^ b)) >> 31 & 1) as u8;
    //     *zf = if c == 0 { 1u8 } else { 0u8 };
    //     *e = c;
    // }
}

impl Default for Devices {
    fn default() -> Self {
        Self {
            f: Fstage {},
            d: Dstage {},
            e: Estage {},
            m: Mstage {},
            w: Wstage {},
            imem: InstructionMemory {
                binary: [0; BIN_SIZE].into(),
            },
            ialign: Align {},
            pc_inc: PCIncrement {},
            // alu: ArithmetcLogicUnit {},
        }
    }
}
