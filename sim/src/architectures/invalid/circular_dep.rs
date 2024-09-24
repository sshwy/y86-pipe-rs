crate::define_stages! {
    /// The whole cycle is a single stage.
    SEQstage s { pc: u64 = 0 }
}

sim_macro::hcl! {
#![hardware = crate::architectures::hardware_seq]
#![program_counter = a]
#![termination = term]

// Circular dependency is not allowed!
u64 a = b;

u64 b = a;

bool term = a == b;

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
