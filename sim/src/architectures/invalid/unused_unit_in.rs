crate::define_stages! {
    /// The whole cycle is a single stage.
    SEQstage s { pc: u64 = 0 }
}

sim_macro::hcl! {

#![hardware = crate::architectures::hardware_seq]
#![program_counter = pc]
#![termination = term]
#![stage_alias(S => s)]

u64 pc = S.pc -> imem.pc;

// ALU requires 3 inputs, but only 1 is provided!
u64 aluA = S.pc -> alu.a;

bool term = pc == 0;
}

impl crate::framework::PipeSim<Arch> {
    fn print_state(&self) {}
}

#[cfg(test)]
mod tests {
    use super::Arch;
    use crate::framework::CpuArch;

    #[test]
    #[should_panic]
    fn test_invalid() {
        Arch::build_circuit();
    }
}
