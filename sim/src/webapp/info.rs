//! data records during execution

use crate::{architectures::Signals, object::SourceInfo};
use anyhow::Result;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[derive(Debug, Clone, serde::Serialize)]
pub struct StageInfo {
    pub(crate) tunnels: Vec<&'static str>,
}

/// record of an instruction at different stage
#[wasm_bindgen]
#[derive(Debug, Clone, serde::Serialize)]
pub struct InstInfo {
    // after halt, the pc may come to invalid place
    pub(crate) addr: Option<u64>,
    pub(crate) fetch: Option<StageInfo>,
    pub(crate) decode: Option<StageInfo>,
    pub(crate) execute: Option<StageInfo>,
    pub(crate) memory: Option<StageInfo>,
    pub(crate) writeback: Option<StageInfo>,
}

impl InstInfo {
    pub fn new(sigs: &Signals, src: &[SourceInfo]) -> Result<Self> {
        let src_info = src.iter().find(|o| {
            if let Some(addr) = o.addr {
                addr == sigs.2.f_pc
            } else {
                false
            }
        });
        Ok(Self {
            addr: src_info.map(|a| a.addr).unwrap_or_default(),
            fetch: None,
            decode: None,
            execute: None,
            memory: None,
            writeback: None,
        })
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CycleInfo {
    pub signals: Signals,
    pub cycle_id: u64,
    pub tunnels: Vec<&'static str>, // todo: add unit info
}
