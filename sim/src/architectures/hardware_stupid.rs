//! This module defines hardware units used in the stupid pipeline.

use std::cell::RefCell;
use std::rc::Rc;

use crate::framework::HardwareUnits;
use crate::isa::op_code::*;
use crate::{define_units, framework::MEM_SIZE};

define_units! {
    // stage registers and default values
    PipeRegs {
        /// Fetch stage registers.
        Fstage f {
            pc: u64 = 0
        }
    }

    FunctionalUnits {
        InstructionMemory imem { // with split
            .input(
                /// Given the current PC, it returns the lower 4 bits and higher
                /// 4 bits of that byte.
                ///
                /// If pc exceeds the memory size, set error to true.
                pc: u64)
            .output(
                lower: u8, higher: u8, error: bool
            )
            binary: Rc<RefCell<[u8; MEM_SIZE]>>
        } {
            let binary: &[u8; MEM_SIZE] = &binary.borrow();
            if pc >= MEM_SIZE as u64 {
                *error = true;
            } else {
                let pc = pc as usize;
                let higher_lower = binary[pc];
                *higher = higher_lower >> 4;
                *lower = higher_lower & 0xf;
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
            alu: ArithmetcLogicUnit {},
        }
    }

    fn mem(&self) -> [u8; MEM_SIZE] {
        *self.imem.binary.borrow()
    }

    fn registers(&self) -> Vec<(u8, u64)> {
        Vec::new()
    }
}
