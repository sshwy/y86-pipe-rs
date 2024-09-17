//! This pipeline is very stupid. It scan the higher 4 bit of every byte and
//! halt if they are all 0 (the halt instruction code).
//!
//! Obviously this pipeline does not follow the RISC-V ISA, so we prepare
//! a special stupid "instruction memory" for it :)

use crate::framework::CpuSim;

hcl_macro::hcl! {

#![hardware = crate::architectures::hardware_stupid]
#![program_counter = f_pc]
#![termination = term]

// Although this is a very stupid pipeline, we still have a pipeline register.
// The stage is still named "Fetch Stage", abbreviated as "F".
#![stage_alias(F => f)]

// The `pc` stored in F are used as the index of current byte. We pass it
// to the instruction memory.
// todo: fix pc name!
u64 f_pc = F.pc -> i.imem.pc;

// We get the instruction code (the higher 4 bits expanded unsignedly to 8 bits).
u8 icode = o.imem.higher;

// Note that we can not simply write `u64 next_pc = F.pc + 1` to get the next
// pc. We have to use ALU to do the addition.
u64 alua = F.pc -> i.alu.a;
u64 alub = 1 -> i.alu.b;
u8 alufun = ADD -> i.alu.fun;

// We get the next pc and pass it to the next stage.
u64 next_pc = o.alu.e -> f.pc;

// If the instruction code is 0, we halt.
bool term = icode == 0;

}

impl crate::framework::PipeSim<Arch> {
    fn print_state(&self) {
        println!(
            "cycle: {}  pc: {:x}  icode {}",
            self.cycle_count(),
            self.program_counter(),
            self.cur_inter.icode,
        );
    }
}
