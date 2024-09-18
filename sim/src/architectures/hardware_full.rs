//! This module defines hardware units used in the classic RISC-V pipeline.
//! The units are defined using the `define_units!` macro.

use std::cell::RefCell;
use std::rc::Rc;

use crate::framework::HardwareUnits;
use crate::isa::cond_fn::*;
use crate::isa::inst_code::NOP;
use crate::isa::op_code::*;
use crate::isa::reg_code;
use crate::isa::reg_code::*;
use crate::utils::format_reg_val;
use crate::{
    define_units,
    framework::MEM_SIZE,
    utils::{get_u64, put_u64},
};

/// Simulator State (at each stage), depending on the hardware design.
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum Stat {
    /// Indicates that everything is fine.
    Aok = 0,
    /// Indicates that the stage is bubbled. A bubbled stage execute the NOP instruction.
    /// Initially, all stages are in the bubble state.
    Bub = 1,
    /// The halt state. This state is assigned when the instruction fetcher reads
    /// the halt instruction. (If your architecture lacks a instruction fetcher,
    /// there should be some other way to specify the halt state in HCL.)
    Hlt = 2,
    /// This state is assigned when the instruction memory or data memory is accessed
    /// with an invalid address.
    Adr = 3,
    /// This state is assigned when the instruction fetcher reads an invalid instruction
    /// code.
    Ins = 4,
}

impl Default for Stat {
    fn default() -> Self {
        Self::Aok
    }
}

define_units! {
    // stage registers and default values for bubble status
    PipeRegs {
        /// Fetch stage registers.
        /// note that it's not possible to bubble (see hcl)
        Fstage f {
            pred_pc: u64 = 0
        }
        Dstage d {
            stat: Stat = Stat::Bub, icode: u8 = NOP, ifun: u8 = 0,
            ra: u8 = RNONE, rb: u8 = RNONE, valc: u64 = 0, valp: u64 = 0
        }
        Estage e {
            stat: Stat = Stat::Bub, icode: u8 = NOP, ifun: u8 = 0,
            vala: u64 = 0, valb: u64 = 0, valc: u64 = 0, dste: u8 = RNONE,
            dstm: u8 = RNONE, srca: u8 = RNONE, srcb: u8 = RNONE
        }
        /// Memory Access Stage
        Mstage m {
            stat: Stat = Stat::Bub, icode: u8 = NOP, cnd: bool = false,
            vale: u64 = 0, vala: u64 = 0, dste: u8 = RNONE, dstm: u8 = RNONE
        }
        Wstage w {
            stat: Stat = Stat::Bub, icode: u8 = NOP, vale: u64 = 0,
            valm: u64 = 0, dste: u8 = RNONE, dstm: u8 = RNONE
        }

    }

    FunctionalUnits {
        InstructionMemory imem { // with split
            .input(pc: u64)
            .output(
                /// This signal is set to true if the address is invalid.
                /// (i.e. the address is out of the memory range)
                error: bool, icode: u8, ifun: u8, align: [u8; 9]
            )
            binary: Rc<RefCell<[u8; MEM_SIZE]>>
        } {
            let binary: &[u8; MEM_SIZE] = &binary.borrow();
            if pc + 10 > MEM_SIZE as u64 {
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
            .output(
                /// The new PC value computed based on need_valc and need_regids.
                new_pc: u64
            )
        } {
            let mut x = old_pc + 1;
            if need_regids { x += 1; }
            if need_valc { x += 8; }
            *new_pc = x;
        }

        /// The register file perform two tasks:
        /// 
        /// 1. Read the values of the source registers `srca` and `srcb`.
        /// 2. Write the value of the destination register `dste` and `dstm`.
        /// 
        /// If the register is `RNONE`, the corresponding operation is not performed.
        RegisterFile reg_file {
            .input(srca: u8, srcb: u8, dste: u8, dstm: u8, vale: u64, valm: u64)
            .output(vala: u64, valb: u64)
            state: [u64; 16]
        } {
            // if RNONE, set to 0 for better debugging
            *vala = if srca != RNONE { state[srca as usize] } else { 0 };
            *valb = if srcb != RNONE { state[srcb as usize] } else { 0 };
            if dste != RNONE {
                tracing::info!("write back fron e: dste = {}, vale = {:#x}", reg_code::name_of(dste), vale);
                state[dste as usize] = vale;
            }
            if dstm != RNONE {
                tracing::info!("write back fron m: dstm = {}, valm = {:#x}", reg_code::name_of(dstm), valm);
                state[dstm as usize] = valm;
            }
        }

        ArithmetcLogicUnit alu {
            .input(a: u64, b: u64, fun: u8)
            .output(e: u64)
        } {
            *e = match fun {
                ADD => b.wrapping_add(a),
                SUB => b.wrapping_sub(a),
                AND => b & a,
                XOR => b ^ a,
                _ => 0,
            };
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
            tracing::info!("CC: a = {:#x}, b = {:#x}, e = {:#x}, sf = {sf}, of = {of}, zf = {zf}", a, b, e);
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
            .output(
                /// If `read == true`, this signal is the data read from memory.
                /// Otherwise this signal is set to 0.
                dataout: u64,
                /// Indicate if the address is invalid.
                error: bool
            )
            binary: Rc<RefCell<[u8; MEM_SIZE]>>
        } {
            if addr + 8 >= MEM_SIZE as u64 {
                *dataout = 0;
                *error = true;
                return
            }
            *error = false;
            if write {
                tracing::info!("write memory: addr = {:#x}, datain = {:#x}", addr, datain);
                let section: &mut [u8] = &mut binary.borrow_mut()[(addr as usize)..];
                put_u64(section, datain);
                *dataout = 0;
            } else if read {
                *dataout = get_u64(&binary.borrow()[(addr as usize)..]);
            }
        }
    }
}

impl HardwareUnits for Units {
    /// Init CPU harewre with given memory.
    fn init(memory: [u8; MEM_SIZE]) -> Self {
        let cell = std::rc::Rc::new(RefCell::new(memory));
        Self {
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

    fn mem(&self) -> [u8; MEM_SIZE] {
        *self.dmem.binary.borrow()
    }

    fn registers(&self) -> Vec<(u8, u64)> {
        self.reg_file
            .state
            .iter()
            .enumerate()
            .filter(|(id, _)| (*id as u8) != RNONE)
            .map(|(i, &v)| (i as u8, v))
            .collect()
    }
}

impl Units {
    pub(crate) fn print_reg(&self) -> String {
        format!(
            "ax {rax} bx {rbx} cx {rcx} dx {rdx}\nsi {rsi} di {rdi} sp {rsp} bp {rbp}",
            rax = format_reg_val(self.reg_file.state[RAX as usize]),
            rbx = format_reg_val(self.reg_file.state[RBX as usize]),
            rcx = format_reg_val(self.reg_file.state[RCX as usize]),
            rdx = format_reg_val(self.reg_file.state[RDX as usize]),
            rsi = format_reg_val(self.reg_file.state[RSI as usize]),
            rdi = format_reg_val(self.reg_file.state[RDI as usize]),
            rsp = format_reg_val(self.reg_file.state[RSP as usize]),
            rbp = format_reg_val(self.reg_file.state[RBP as usize]),
        )
    }
}
