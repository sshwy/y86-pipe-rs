// Test single instructions in pipeline

use std::collections::HashMap;

use interpolator::{format, Formattable};

#[allow(non_upper_case_globals)]
const vals: [i64; 3] = [0x100, 0x020, 0x004];

fn make_obj(src: &str) -> anyhow::Result<y86_pipe_rs::ObjectExt> {
    let obj = y86_pipe_rs::assemble(
        src,
        y86_pipe_rs::AssembleOption::default().set_verbose(false),
    )?;

    Ok(obj)
}

#[test]
fn test_reg_op() -> anyhow::Result<()> {
    let insts = ["rrmovq", "addq", "subq", "andq", "xorq"];
    let regs = ["%rdx", "%rbx", "%rsp"];
    let source = r#"
        irmovq ${vala}, {ra}
        irmovq ${valb}, {rb}
        nop
        nop
        nop
        {inst} {ra}, {rb}
        nop
        nop
        halt
    "#;

    for inst in insts {
        for ra in regs {
            for rb in regs {
                let args = &[
                    ("vala", Formattable::display(&vals[0])),
                    ("valb", Formattable::display(&vals[1])),
                    ("ra", Formattable::display(&ra)),
                    ("rb", Formattable::display(&rb)),
                    ("inst", Formattable::display(&inst)),
                ]
                .into_iter()
                .collect::<HashMap<_, _>>();

                let src = format(source, args)?;
                let obj = make_obj(&src)?;
                let mut pipe = y86_pipe_rs::pipeline::Simulator::new(
                    <y86_pipe_rs::Arch as y86_pipe_rs::pipeline::CpuArch>::Units::init(
                        obj.obj.binary,
                    ),
                );
                while !pipe.is_terminate() {
                    pipe.step();
                }
                anyhow::ensure!(pipe.is_success(), "test failed: op-{inst}-{ra}-{rb}");
            }
        }
    }

    Ok(())
}
