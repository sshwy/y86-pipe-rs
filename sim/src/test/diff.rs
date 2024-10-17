//! Test the architecture by comparing the simulation result with a "ground
//! truth".

use anyhow::Context;

use super::SimTester;
use crate::{
    framework::{CpuSim, MemData},
    isa::StandardResult,
};

pub struct TestIsaResult {
    pub answer: StandardResult,
    pub sim: Box<dyn CpuSim>,
    pub mem: MemData,
}

impl TestIsaResult {
    fn check(&self) -> anyhow::Result<()> {
        let gt_regs = self.answer.regs;
        let sim_regs = self.sim.registers();
        if gt_regs != sim_regs {
            anyhow::bail!(
                "registers mismatch: gt = {:?}, sim = {:?}",
                gt_regs,
                sim_regs
            );
        }

        let gt_mem = self.answer.bin;
        let sim_mem_read = self.mem.read();
        if gt_mem.as_ref() != sim_mem_read.as_ref() {
            crate::utils::mem_diff(&gt_mem, &sim_mem_read);
            anyhow::bail!("memory mismatch");
        }
        drop(sim_mem_read);
        Ok(())
    }
}

impl SimTester {
    pub fn test_isa(&self, src: &str) -> anyhow::Result<TestIsaResult> {
        let a = super::make_obj(src).context("assemble")?;
        let answer = crate::isa::simulate(a.obj.init_mem(), false)?;
        let (sim, sim_mem) = SimTester::simulate_arch(self.arch.clone(), src)?;

        let r = TestIsaResult {
            answer,
            sim,
            mem: sim_mem,
        };

        r.check()?;

        Ok(r)
    }
}
