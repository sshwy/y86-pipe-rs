//! Test the architecture by comparing the simulation result with a "ground
//! truth".

use anyhow::Context;

use super::SimTester;

impl SimTester {
    pub fn test_compare(&self, gt_arch: String, src: &str) -> anyhow::Result<()> {
        let (gt, gt_mem) = SimTester::simulate_arch(gt_arch, src).context("simulate gt")?;
        let (sim, sim_mem) = SimTester::simulate_arch(self.arch.clone(), src)?;

        let gt_regs = gt.registers();
        let sim_regs = sim.registers();
        if gt_regs != sim_regs {
            anyhow::bail!(
                "registers mismatch: gt = {:?}, sim = {:?}",
                gt_regs,
                sim_regs
            );
        }

        let gt_mem_read = gt_mem.read();
        let sim_mem_read = sim_mem.read();
        if gt_mem_read.as_ref() != sim_mem_read.as_ref() {
            crate::utils::mem_diff(&gt_mem_read, &sim_mem_read);
            anyhow::bail!("memory mismatch");
        }

        Ok(())
    }
}
