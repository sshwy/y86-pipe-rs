mod propagate;

pub use propagate::{PropCircuit, PropOrder, PropOrderBuilder, PropUpdates, Propagator, Tracer};

/// Pipeline State
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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

pub enum CpuStatus {
    CycleStart,
    CycleEnd,
}

/// During a CPU cycle, signals in memory devices (stage units) are propagated through
/// the combinational logic circuits. The signals are then latched into the pipeline
/// registers at the end of the cycle. Therefore we can use two basic operations to
/// simulate the pipeline.
trait CpuSim {
    type UnitInputSignals;
    type UnitOutputSignals;
    /// In the pipeline, we have specific memory devices to store data.
    /// Stage data are signals that passed between CPU cycles.
    type StageData;

    /// Initiate the next cycle or the first cycle. This function should be called
    /// at the very beginning of the simulation, or after calling [`CpuSim::propagate_signals`].
    /// Otherwise the behavior is undefined.
    fn initiate_next_cycle(&mut self);

    /// Propagate signals through the combinational logic circuits. This function
    /// should be called after [`CpuSim::initiate_next_cycle`]. Otherwise the
    /// behavior is undefined.
    fn propagate_signals(&mut self);
}

// here we use trait to collect the types
pub trait CpuCircuit {
    type UnitIn;
    type UnitOut;
    type Inter;
}

pub trait CpuArch: CpuCircuit {
    type Units;
}

pub type Signals<A> = (
    <A as CpuCircuit>::UnitIn,
    <A as CpuCircuit>::UnitOut,
    <A as CpuCircuit>::Inter,
);

/// pipeline runner
pub struct Pipeline<T: CpuArch> {
    pub(crate) circuit: PropCircuit<T>,
    /// signals are returned after each step, thus set to private
    pub(crate) cur_unit_in: T::UnitIn,
    pub(crate) cur_unit_out: T::UnitOut,
    pub(crate) cur_inter: T::Inter,
    /// units are not easily made clone, thus it's up to app to decide which information to save.
    pub(crate) units: T::Units,
    /// we have [`is_terminate`]
    pub(crate) terminate: bool,
}

impl<T: CpuArch> Pipeline<T> {
    pub fn is_terminate(&self) -> bool {
        self.terminate
    }
}
