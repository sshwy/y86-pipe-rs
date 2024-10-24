//! This module contains utilities for verifying the correctness of an
//! architecture's implementation.

mod diff;
mod inst;

use crate::framework::{CpuSim, MemData};

pub struct SimTester {
    arch: String,
}

impl SimTester {
    pub fn new(arch: &str) -> Option<Self> {
        if crate::architectures::arch_names()
            .iter()
            .any(|&a| a == arch)
        {
            Some(Self { arch: arch.into() })
        } else {
            None
        }
    }

    /// Simulate the given source code and return the simulator and the memory
    /// after the simulation.
    fn simulate_arch(arch: String, src: &str) -> anyhow::Result<(Box<dyn CpuSim>, MemData)> {
        let obj = make_obj(src)?;
        let mem = MemData::init(obj.obj.init_mem());
        let mut pipe = crate::architectures::create_sim(arch, mem.clone(), false);
        while !pipe.is_terminate() {
            pipe.step();
            if pipe.cycle_count() > 3_000_000 {
                anyhow::bail!("exceed maximum CPU cycle limit");
            }
        }
        Ok((pipe, mem))
    }
}

fn make_obj(src: &str) -> anyhow::Result<crate::ObjectExt> {
    let obj = crate::assemble(src, crate::AssembleOption::default().set_verbose(false))?;

    Ok(obj)
}
