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
    // pipline
    // Pip = 5,
}

impl Default for Stat {
    fn default() -> Self {
        Self::Aok
    }
}
