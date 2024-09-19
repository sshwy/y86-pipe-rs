// All hardware modules
pub mod hardware_full;
pub mod hardware_seq;
pub mod hardware_stupid;

// This module is not public because no one should use it.
mod pipe_invalid;

// Builtin pipeline architectures
mod builtin;
pub use builtin::seq_std;

// Extra pipeline architectures
mod extra;
pub use extra::ARCH_NAMES as EXTRA_ARCH_NAMES;

use crate::framework::{CpuSim, MemData, PipeSim};

/// Get all architecture names
pub fn arch_names() -> Vec<&'static str> {
    let mut names = vec!["seq_std"];
    names.extend(extra::ARCH_NAMES);
    names
}

pub fn create_sim(kind: String, memory: MemData, tty_out: bool) -> Box<dyn CpuSim> {
    match kind.as_str() {
        "seq_std" => Box::new(PipeSim::<builtin::seq_std::Arch>::new(memory, tty_out)),
        _ => extra::create_sim(kind, memory, tty_out),
    }
}
