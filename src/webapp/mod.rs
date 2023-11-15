//! Pipeline simulator for the web

use crate::{
    assemble,
    object::{ObjectExt, SourceInfo},
    pipeline::pipe_full::Signals,
    record::TransLog,
    Pipeline,
};
use anyhow::Context;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct App {
    obj: ObjectExt,
    pipe: Pipeline,
    records: Vec<(Signals, TransLog)>,
}

#[wasm_bindgen]
impl SourceInfo {
    pub fn addr(&self) -> Option<u64> {
        self.addr
    }
    pub fn source(&self) -> String {
        self.src.clone()
    }
}

#[wasm_bindgen]
impl App {
    #[wasm_bindgen(constructor)]
    pub fn new(src: &str) -> Result<App, String> {
        let obj = assemble(src, Default::default())
            .context("assemble source file")
            .map_err(|e| e.to_string())?;
        let pipe = Pipeline::init(obj.obj.binary);
        Ok(App {
            obj,
            pipe,
            records: Default::default(),
        })
    }
    /// step the simulator, return true if is terminated
    pub fn step(&mut self) -> bool {
        let r: (Signals, TransLog) = self.pipe.step();
        self.records.push(r);
        // todo: get device info
        return self.pipe.is_terminate();
    }
    pub fn src_info(&self) -> Vec<SourceInfo> {
        self.obj.source.clone()
    }
}
