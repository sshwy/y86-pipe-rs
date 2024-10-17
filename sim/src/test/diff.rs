//! Test the architecture by comparing the simulation result with a "ground
//! truth".

use anyhow::Context;

use super::SimTester;

impl SimTester {
    pub fn test_isa(&self, src: &str) -> anyhow::Result<()> {
        let a = super::make_obj(src).context("assemble")?;
        let answer = crate::isa::simulate(a.obj.init_mem())?;
        let (sim, sim_mem) = SimTester::simulate_arch(self.arch.clone(), src)?;

        let gt_regs = answer.regs;
        let sim_regs = sim.registers();
        if gt_regs != sim_regs {
            anyhow::bail!(
                "registers mismatch: gt = {:?}, sim = {:?}",
                gt_regs,
                sim_regs
            );
        }

        let gt_mem_read = answer.bin;
        let sim_mem_read = sim_mem.read();
        if gt_mem_read.as_ref() != sim_mem_read.as_ref() {
            crate::utils::mem_diff(&gt_mem_read, &sim_mem_read);
            anyhow::bail!("memory mismatch");
        }

        Ok(())
    }
}
