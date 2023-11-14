use crate::record::NameList;

pub mod hardware;
pub mod pipe_full;

/// Pipeline Pipeline State
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum Stat {
    Aok = 0,
    /// bubble
    Bub = 1,
    /// halt
    Hlt = 2,
    /// invalid address
    Adr = 3,
    /// invalid instruction
    Ins = 4,
}

impl Default for Stat {
    fn default() -> Self {
        Self::Aok
    }
}

/// pipeline runner
pub struct Pipeline<Sigs: Default, Devices> {
    pub(crate) order: Option<NameList>,
    /// signals are returned after each step, thus set to private
    runtime_signals: Sigs,
    /// devices are not easily made clone, thus it's up to app to decide which information to save.
    pub(crate) devices: Devices,
    /// we have [`is_terminate`]
    terminate: bool,
}

impl<Sig: Default, Devices> Pipeline<Sig, Devices> {
    pub fn is_terminate(&self) -> bool {
        self.terminate
    }
}
