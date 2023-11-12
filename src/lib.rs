mod asm;
mod isa;
mod object;

pub use asm::assemble;
pub use asm::AssembleOption;

#[cfg(test)]
mod tests {
    use crate::{assemble, AssembleOption};

    #[test]
    fn test_assemble() {
        let r = assemble(crate::asm::tests::RSUM_YS, AssembleOption::default()).unwrap();
        dbg!(&r.source);
        eprintln!("{}", r);
    }
}
