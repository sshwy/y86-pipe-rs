//! This module contains utilities for verifying the correctness of an
//! architecture's implementation.

mod inst;

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

    fn simulate(&self, src: &str) -> anyhow::Result<Box<dyn crate::framework::CpuSim>> {
        let obj = make_obj(&src)?;
        let mem = crate::framework::MemData::init(obj.obj.init_mem());
        let mut pipe = crate::architectures::create_sim(self.arch.clone(), mem, false);
        while !pipe.is_terminate() {
            pipe.step();
            if pipe.cycle_count() > 3000_000 {
                anyhow::bail!("exceed maximum CPU cycle limit");
            }
        }
        Ok(pipe)
    }
}

fn make_obj(src: &str) -> anyhow::Result<crate::ObjectExt> {
    let obj = crate::assemble(src, crate::AssembleOption::default().set_verbose(false))?;

    Ok(obj)
}
