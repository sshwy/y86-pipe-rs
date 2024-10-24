//! Test single instructions in pipeline

use interpolator::format;

use super::{diff::TestIsaResult, SimTester};

#[allow(non_upper_case_globals)]
const vals: [i64; 3] = [0x100, 0x020, 0x004];

macro_rules! interp_args {
    ($( $name:ident = $e:expr ),* $(,)?) => {
        &[
            $( (stringify!($name), interpolator::Formattable::display(&$e)) ),*
        ]
        .into_iter()
        .collect::<std::collections::HashMap<_, _>>()
    };
}

impl SimTester {
    fn test_isa_write_on_fail(&self, src: &str) -> anyhow::Result<TestIsaResult> {
        self.test_isa(src).inspect_err(|_| {
            // Write the failing test case to a file
            std::fs::write("test-failure.ys", src).unwrap();
        })
    }

    pub fn test_opq(&self) -> anyhow::Result<()> {
        let insts = ["addq", "subq", "andq", "xorq"];
        let regs = ["%rdx", "%rbx", "%rsp"];
        let source = r#"
            irmovq ${vala}, {ra}
            irmovq ${valb}, {rb}
            nop
            nop
            nop
            nop
            nop
            {inst} {ra}, {rb}
            nop
            nop
            nop
            nop
            nop
            halt
        "#;

        for inst in insts {
            for ra in regs {
                for rb in regs {
                    let src = format(
                        source,
                        interp_args!(
                            vala = vals[0],
                            valb = vals[1],
                            ra = ra,
                            rb = rb,
                            inst = inst
                        ),
                    )?;
                    self.test_isa_write_on_fail(&src)?;
                }
            }
        }

        Ok(())
    }

    pub fn test_iopq(&self) -> anyhow::Result<()> {
        let insts = ["iaddq", "isubq", "iandq", "ixorq"];
        let source = r#"
            irmovq ${valb}, %rdx
            nop
            nop
            nop
            nop
            nop
            {inst} {vala}, %rdx
            nop
            nop
            nop
            nop
            nop
            halt
        "#;

        for inst in insts {
            let src = format(
                source,
                interp_args!(vala = vals[0], valb = vals[1], inst = inst),
            )?;
            self.test_isa_write_on_fail(&src)?;
        }

        Ok(())
    }

    pub fn test_cmov(&self) -> anyhow::Result<()> {
        let insts = [
            "rrmovq", "cmovle", "cmovl", "cmove", "cmovne", "cmovge", "cmovg",
        ];

        let source = r#"
            irmovq ${vala}, %rdi
            irmovq ${valb}, %rsi
            xorq %rdx, %rdx
            subq %rdi, %rsi
            {inst} %rdi, %rdx
            halt
        "#;

        for inst in insts {
            for valb in vals {
                let src = format(
                    source,
                    interp_args!(vala = vals[1], valb = valb, inst = inst),
                )?;
                self.test_isa_write_on_fail(&src)?;
            }
        }

        Ok(())
    }

    pub fn test_jm(&self) -> anyhow::Result<()> {
        let insts = ["jmp", "jle", "jl", "je", "jne", "jge", "jg"];

        let source = r#"
            irmovq ${vala}, %rdi
            irmovq ${valb}, %rsi
            xorq %rdx, %rdx
            subq %rdi, %rsi
            {inst} L1
            rrmovq %rdi, %rdx
        L1:
            halt
        "#;

        for inst in insts {
            for valb in vals {
                let src = format(
                    source,
                    interp_args!(vala = vals[1], valb = valb, inst = inst),
                )?;
                self.test_isa_write_on_fail(&src)?;
            }
        }

        Ok(())
    }
}
