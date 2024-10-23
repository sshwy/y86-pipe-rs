//! This module defines hardware units used in the seq architecture.

use std::{cell::RefCell, rc::Rc};

use crate::{
    define_units,
    framework::{HardwareUnits, MemData, MEM_SIZE},
    isa::{
        inst_code,
        reg_code::{self, *},
        RegFile,
    },
    utils::{get_u64, put_u64},
};

/// A constant that represents the value -8.
pub const NEG_8: u64 = -8i64 as u64;
pub use crate::isa::{ConditionCode, Stat, CC_INIT};

define_units! {
    InstructionMemory imem { // with split
        .input(
            /// The input pc is used to read the instruction from memory.
            pc: u64
        )
        .output(
            /// This signal is set to true if the address is invalid.
            /// (i.e. the address is out of the memory range)
            error: bool, icode: u8, ifun: u8, align: [u8; 9]
        )
        binary: MemData
    } {
        let binary: &[u8; MEM_SIZE] = &binary.read();
        if pc + 10 > MEM_SIZE as u64 {
            *error = true;
        } else {
            let pc = pc as usize;
            let icode_ifun = binary[pc];
            *icode = icode_ifun >> 4;
            *ifun = icode_ifun & 0xf;
            *align = binary[pc+1..pc+10].try_into().unwrap();
        }

        if *icode == inst_code::CALL {
            tracing::info!("CALL instruction fetched");
        }
    }

    /// If `need_regids` is set to true, this unit will extract the register
    /// IDs from the first byte, and valC from the rest of the bytes.
    /// Otherwise the valC is extracted from the first 8 bytes and the last byte
    /// is ignored.
    Align ialign {
        .input(need_regids: bool, align: [u8; 9])
        .output(rA: u8, rB: u8,
            /// Constant value extracted from the instruction. If the instruction
            /// does not need a constant value, this signal is meaningless.
            valC: u64)
    } {
        let ra_rb = align[0];
        let rest = if need_regids {
            *rA = ra_rb >> 4;
            *rB = ra_rb & 0xf;
            &align[1..9]
        } else {
            *rA = RNONE;
            *rB = RNONE;
            &align[0..8]
        };
        *valC = get_u64(rest)
    }

    PCIncrement pc_inc {
        .input(need_valC: bool, need_regids: bool, old_pc: u64)
        .output(
            /// The new PC value computed based on need_valC and need_regids.
            new_pc: u64
        )
    } {
        let mut x = old_pc + 1;
        if need_regids { x += 1; }
        if need_valC { x += 8; }
        *new_pc = x;
    }

    RegisterFileRead reg_read {
        .input(srcA: u8, srcB: u8)
        .output(valA: u64, valB: u64)
        state: Rc<RefCell<RegFile>>
    } {
        // if RNONE, set to 0 for better debugging
        let state  = &mut state.borrow_mut();
        *valA = if srcA != RNONE { state[srcA as usize] } else { 0 };
        *valB = if srcB != RNONE { state[srcB as usize] } else { 0 };
    }

    RegisterFileWrite reg_write {
        .input(dstE: u8, dstM: u8, valE: u64, valM: u64)
        .output()
        state: Rc<RefCell<RegFile>>
    } {
        let state  = &mut state.borrow_mut();
        if dstE != RNONE {
            tracing::info!("write back fron e: dstE = {}, valE = {:#x}", reg_code::name_of(dstE), valE);
            state[dstE as usize] = valE;
        }
        if dstM != RNONE {
            tracing::info!("write back fron m: dstM = {}, valM = {:#x}", reg_code::name_of(dstM), valM);
            state[dstM as usize] = valM;
        }
    }

    ArithmetcLogicUnit alu {
        .input(a: u64, b: u64, fun: u8)
        .output(e: u64)
    } {
        *e = crate::isa::arithmetic_compute(a, b, fun).unwrap_or(0);
    }

    /// Given the input and output of the ALU, this unit calculate the
    /// condition codes and update the cc register if required.
    RegisterCC reg_cc {
        .input(set_cc: bool, a: u64, b: u64, e: u64, opfun: u8)
        .output(cc: ConditionCode)
        inner_cc: ConditionCode
    } {
        if set_cc {
            inner_cc.set(a, b, e, opfun);
            tracing::info!("CC update: a = {:#x}, b = {:#x}, e = {:#x}, cc: {:?}, opfun: {}", a, b, e,
                inner_cc, crate::isa::op_code::name_of(opfun));
        }
        *cc = *inner_cc;
    }

    /// Instructions like CMOVX or JX needs to check the condition code based
    /// on the function code, which is simulated by this unit.
    InstructionCondition cond {
        .input(condfun: u8, cc: ConditionCode)
        .output(cnd: bool)
    } {
        *cnd = cc.test(condfun);
    }

    DataMemory dmem {
        .input(addr: u64, datain: u64, read: bool, write: bool)
        .output(
            /// If `read == true`, this signal is the data read from memory.
            /// Otherwise this signal is set to 0.
            dataout: u64,
            /// Indicate if the address is invalid.
            error: bool
        )
        binary: MemData
    } {
        if addr + 8 >= MEM_SIZE as u64 {
            *dataout = 0;
            *error = true;
            return
        }
        *error = false;
        if write {
            tracing::info!("write memory: addr = {:#x}, datain = {:#x}", addr, datain);
            let section: &mut [u8] = &mut binary.write()[(addr as usize)..];
            put_u64(section, datain);
            *dataout = 0;
        } else if read {
            *dataout = get_u64(&binary.read()[(addr as usize)..]);
        }
    }
}

impl std::fmt::Display for Units {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let reg_file = self.reg_read.state.borrow();
        let regs = crate::utils::format_reg_file(*reg_file);
        f.write_fmt(format_args!("{regs}\n{cc}", cc = self.reg_cc.inner_cc))
    }
}

impl HardwareUnits for Units {
    /// Init CPU harewre with given memory.
    fn init(memory: MemData) -> Self {
        let reg = Rc::new(RefCell::new([0; 16]));
        Self {
            imem: InstructionMemory {
                binary: memory.clone(),
            },
            ialign: Align {},
            pc_inc: PCIncrement {},
            reg_read: RegisterFileRead { state: reg.clone() },
            reg_write: RegisterFileWrite { state: reg.clone() },
            alu: ArithmetcLogicUnit {},
            reg_cc: RegisterCC {
                inner_cc: ConditionCode::default(),
            },
            cond: InstructionCondition {},
            dmem: DataMemory { binary: memory },
        }
    }

    fn register_file(&self) -> RegFile {
        self.reg_read.state.borrow().clone()
    }
}
