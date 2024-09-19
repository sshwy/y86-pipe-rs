sim_macro::hcl! {

#![hardware = crate::architectures::hardware_stupid]
#![program_counter = pc]
#![termination = term]
#![stage_alias(F => f)]

u64 pc = F.pc -> imem.pc;
u8 icode = imem.higher;

// ALU requires 3 inputs, but only 1 is provided!
u64 alua = F.pc -> alu.a;

bool term = icode == 0;
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
