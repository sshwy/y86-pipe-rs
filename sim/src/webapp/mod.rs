//! Pipeline simulator for the web

mod error;
mod info;

use self::error::AppError;
use self::info::{CycleInfo, InstInfo};
use crate::architectures::Signals;
use crate::{
    assemble, object::ObjectExt, record::Tracer, webapp::info::StageInfo,
    DefaultPipeline as Pipeline,
};
use anyhow::Context;
use anyhow::Result;
use serde::Serialize;
use serde_wasm_bindgen::Serializer;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct App {
    obj: ObjectExt,
    pipe: Pipeline,
    inst_info: Vec<InstInfo>,
    cycle_info: Vec<CycleInfo>,
    serailzer: serde_wasm_bindgen::Serializer,
}

const DEFAULT_SOURCE: &str = r#"
# a simple a + b program
    .pos 0
    irmovq input, %rdi
    mrmovq (%rdi), %rdx
    mrmovq 8(%rdi), %rax
    addq %rdx, %rax
    halt
    .align 8
input:
    .quad 0x1234
    .quad 0x4321
"#;

#[wasm_bindgen]
impl App {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<App, AppError> {
        let obj = assemble(DEFAULT_SOURCE, Default::default()).context("assemble source file")?;
        let pipe = Pipeline::init(obj.obj.binary);
        Ok(App {
            obj,
            pipe,
            inst_info: Default::default(),
            cycle_info: Default::default(),
            serailzer: Serializer::new().serialize_large_number_types_as_bigints(true),
        })
    }
    pub fn is_terminated(&self) -> bool {
        self.pipe.is_terminate()
    }
    /// step the simulator, return changes of each stage
    pub fn step(&mut self) -> Result<JsValue, AppError> {
        let (sigs, logs): (Signals, Tracer) = self.pipe.step();

        // update instinfos
        self.inst_info.push(InstInfo::new(&sigs, &self.obj.source)?);
        let mut it = self.inst_info.iter_mut().rev().take(5);

        macro_rules! tun_filter {
            ($tunnel:expr, $suf:literal) => {
                $tunnel
                    .iter()
                    .copied()
                    .filter(|s| s.ends_with($suf))
                    .collect()
            };
        }

        if let Some(inst) = it.next() {
            inst.fetch = Some(StageInfo {
                tunnels: tun_filter!(logs.tunnel, "FF"),
            });
        }
        if let Some(inst) = it.next() {
            inst.decode = Some(StageInfo {
                tunnels: tun_filter!(logs.tunnel, "DD"),
            });
        }
        if let Some(inst) = it.next() {
            inst.execute = Some(StageInfo {
                tunnels: tun_filter!(logs.tunnel, "EE"),
            });
        }
        if let Some(inst) = it.next() {
            inst.memory = Some(StageInfo {
                tunnels: tun_filter!(logs.tunnel, "MM"),
            });
        }
        if let Some(inst) = it.next() {
            inst.writeback = Some(StageInfo {
                tunnels: tun_filter!(logs.tunnel, "WW"),
            });
        }

        let cycle_id = self.cycle_info.len() as u64;
        let c = CycleInfo {
            cycle_id,
            signals: sigs,
            tunnels: logs.tunnel,
        };
        let r = c.serialize(&self.serailzer)?; // serde_wasm_bindgen::to_value(&c)?;
        self.cycle_info.push(c);
        Ok(r)
    }
    pub fn instructions(&self) -> Result<JsValue, AppError> {
        Ok(self.inst_info.serialize(&self.serailzer)?)
    }
}

#[cfg(test)]
mod tests {
    use super::App;

    #[test]
    fn test_app() {
        let mut app = App::new().unwrap();
        while !app.is_terminated() {
            let r = app.step().unwrap();
            dbg!(r);
        }
    }
}
