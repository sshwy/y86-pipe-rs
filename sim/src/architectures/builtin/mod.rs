//! Builtin architectures
pub mod pipe_std;
pub mod seq_plus_std;
pub mod seq_std;

#[cfg(test)]
mod tests {
    use crate::test::SimTester;

    #[test]
    fn test_builtin() -> anyhow::Result<()> {
        for tester in [
            SimTester::new("seq_std").unwrap(),
            SimTester::new("seq_plus_std").unwrap(),
        ]
        .into_iter()
        {
            tester.test_opq()?;
            tester.test_cmov()?;
            tester.test_jm()?;
        }

        Ok(())
    }
}
