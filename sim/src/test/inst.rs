//! Test single instructions in pipeline

use interpolator::format;

use super::SimTester;
use crate::asm::Reg;

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

macro_rules! test_ensure {
    ($name:literal, $src:expr, $ans:expr, $res:expr ) => {
        if ($res) != ($ans) {
            std::fs::write("test-failure.ys", $src)?;
            anyhow::bail!(
                "test failed: {name}, expected: {answer}, got: {result}",
                name = format!($name),
                answer = $ans,
                result = $res
            );
        }
    };
}

impl SimTester {
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

        fn eval(inst: &str, ra: &str, rb: &str) -> i64 {
            if ra != rb {
                match inst {
                    "addq" => vals[1] + vals[0],
                    "subq" => vals[1] - vals[0],
                    "andq" => vals[1] & vals[0],
                    "xorq" => vals[1] ^ vals[0],
                    _ => unreachable!(),
                }
            } else {
                match inst {
                    "addq" => vals[1] + vals[1],
                    "subq" => vals[1] - vals[1],
                    "andq" => vals[1] & vals[1],
                    "xorq" => vals[1] ^ vals[1],
                    _ => unreachable!(),
                }
            }
        }

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

                    let pipe = self.simulate(&src)?;

                    let answer = eval(inst, ra, rb) as u64;
                    let result = pipe
                        .reg(Reg::try_from(rb).unwrap())
                        .ok_or(anyhow::anyhow!("register not found"))?;

                    test_ensure!("opq-{inst}-{ra}-{rb}", src, answer, result);
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

        fn eval(inst: &str) -> i64 {
            match inst {
                "iaddq" => vals[1] + vals[0],
                "isubq" => vals[1] - vals[0],
                "iandq" => vals[1] & vals[0],
                "ixorq" => vals[1] ^ vals[0],
                _ => unreachable!(),
            }
        }

        for inst in insts {
            let src = format(
                source,
                interp_args!(vala = vals[0], valb = vals[1], inst = inst),
            )?;
            let pipe = self.simulate(&src)?;

            let answer = eval(inst) as u64;
            let result = pipe
                .reg(Reg::RDX)
                .ok_or(anyhow::anyhow!("register not found"))?;

            test_ensure!("iopq-{inst}", src, answer, result);
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

        fn eval(inst: &str, vala: i64, valb: i64) -> i64 {
            match inst {
                "rrmovq" => vala,
                "cmovle" => (valb <= vala).then_some(vala).unwrap_or(0),
                "cmovl" => (valb < vala).then_some(vala).unwrap_or(0),
                "cmove" => (valb == vala).then_some(vala).unwrap_or(0),
                "cmovne" => (valb != vala).then_some(vala).unwrap_or(0),
                "cmovge" => (valb >= vala).then_some(vala).unwrap_or(0),
                "cmovg" => (valb > vala).then_some(vala).unwrap_or(0),
                _ => unreachable!(),
            }
        }

        for inst in insts {
            for valb in vals {
                let src = format(
                    source,
                    interp_args!(vala = vals[1], valb = valb, inst = inst),
                )?;
                let pipe = self.simulate(&src)?;
                let answer = eval(inst, vals[1], valb) as u64;
                let result = pipe
                    .reg(Reg::RDX)
                    .ok_or(anyhow::anyhow!("register not found"))?;

                test_ensure!("cmov-{inst}", src, answer, result);
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

        fn eval(inst: &str, vala: i64, valb: i64) -> i64 {
            match inst {
                "jmp" => 0,
                "jle" => (valb <= vala).then_some(0).unwrap_or(vala),
                "jl" => (valb < vala).then_some(0).unwrap_or(vala),
                "je" => (valb == vala).then_some(0).unwrap_or(vala),
                "jne" => (valb != vala).then_some(0).unwrap_or(vala),
                "jge" => (valb >= vala).then_some(0).unwrap_or(vala),
                "jg" => (valb > vala).then_some(0).unwrap_or(vala),
                _ => unreachable!(),
            }
        }

        for inst in insts {
            for valb in vals {
                let src = format(
                    source,
                    interp_args!(vala = vals[1], valb = valb, inst = inst),
                )?;
                let pipe = self.simulate(&src)?;
                let answer = eval(inst, vals[1], valb) as u64;
                let result = pipe
                    .reg(Reg::RDX)
                    .ok_or(anyhow::anyhow!("register not found"))?;

                test_ensure!("jm-{inst}", src, answer, result);
            }
        }

        Ok(())
    }
}
