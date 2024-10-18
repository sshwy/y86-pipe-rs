// All hardware modules
pub mod hardware_pipe;
pub mod hardware_seq;

// Architectures
mod builtin;
mod extra;
mod invalid;

pub use extra::ARCH_NAMES as EXTRA_ARCH_NAMES;

use crate::framework::{CpuSim, MemData, PipeSim};

/// Get all architecture names
pub fn arch_names() -> Vec<&'static str> {
    let mut names = vec!["seq_std", "seq_plus_std", "pipe_std"];
    names.extend(extra::ARCH_NAMES);
    names
}

pub fn create_sim(kind: String, memory: MemData, tty_out: bool) -> Box<dyn CpuSim> {
    match kind.as_str() {
        "seq_std" => Box::new(PipeSim::<builtin::seq_std::Arch>::new(memory, tty_out)),
        "seq_plus_std" => Box::new(PipeSim::<builtin::seq_plus_std::Arch>::new(memory, tty_out)),
        "pipe_std" => Box::new(PipeSim::<builtin::pipe_std::Arch>::new(memory, tty_out)),
        _ => extra::create_sim(kind, memory, tty_out),
    }
}
