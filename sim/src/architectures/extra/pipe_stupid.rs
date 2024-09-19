//! This pipeline is very stupid. It scan the higher 4 bit of every byte and
//! halt if they are all 0 (the halt instruction code).
//!
//! Obviously this pipeline does not follow the RISC-V ISA, so we prepare
//! a special stupid "instruction memory" for it. To handle pc increment, we
//! ship an ALU with it.

sim_macro::hcl! {

#![hardware = crate::architectures::hardware_stupid]
#![program_counter = pc]
#![termination = term]

// Although this is a very stupid pipeline, we still have a pipeline register.
// The stage is still named "Fetch Stage", abbreviated as "F".
#![stage_alias(F => f)]

// The `pc` stored in F are used as the index of current byte. We pass it
// to the instruction memory.
u64 pc = F.pc -> imem.pc;

// We get the instruction code (the higher 4 bits expanded unsignedly to 8 bits).
u8 icode = imem.higher;

// Note that we can not simply write `u64 next_pc = F.pc + 1` to get the next
// pc. We have to use ALU to do the addition.
u64 alua = F.pc -> alu.a;
u64 alub = 1 -> alu.b;
u8 alufun = ADD -> alu.fun;

// We get the next pc and pass it to the next stage.
u64 next_pc = alu.e -> f.pc;

// If the instruction code is 0, we halt.
bool term = icode == 0;

}

impl crate::framework::PipeSim<Arch> {
    fn print_state(&self) {
        println!("icode = {}", self.cur_inter.icode,);
    }
}
