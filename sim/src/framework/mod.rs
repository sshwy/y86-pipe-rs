//! To provide a flexible codebase for different CPU architectures, we give a
//! general CPU simulator framework.
mod propagate;

pub trait HardwareUnits {
    /// A set of hardware units should be initialized from a given memory.
    fn init(memory: [u8; MEM_SIZE]) -> Self;
    /// Get current memory data.
    fn mem(&self) -> [u8; MEM_SIZE];
}

pub use propagate::{PropCircuit, PropOrder, PropOrderBuilder, PropUpdates, Propagator, Tracer};

/// Size of the memory that is used to store instructions and data (stack).
/// No matter what architecture we are using, memory store must exist. Otherwise
/// we have no place to store instructions.
pub const MEM_SIZE: usize = 1 << 20;

pub enum CpuStatus {
    CycleStart,
    CycleEnd,
}

/// During a CPU cycle, signals in memory devices (stage units) are propagated through
/// the combinational logic circuits. The signals are then latched into the pipeline
/// registers at the end of the cycle. Therefore we can use two basic operations to
/// simulate the pipeline.
pub trait CpuSim {
    /// Initiate the next cycle or the first cycle. This function should be called
    /// after calling [`CpuSim::propagate_signals`]. Otherwise the behavior is undefined.
    fn initiate_next_cycle(&mut self);

    /// Propagate signals through the combinational logic circuits. This function
    /// should be called after [`CpuSim::initiate_next_cycle`]. Otherwise the
    /// behavior is undefined.
    fn propagate_signals(&mut self);

    /// Whether the simulation is terminated
    fn is_terminate(&self) -> bool;

    /// Whether the simulation is successfully halted
    fn is_success(&self) -> bool;
}

// here we use trait to collect the types
pub trait CpuCircuit {
    type UnitIn;
    type UnitOut;
    type Inter;
    type StageState;
}

pub trait CpuArch: CpuCircuit {
    type Units: HardwareUnits;
}

pub type Signals<A> = (
    <A as CpuCircuit>::UnitIn,
    <A as CpuCircuit>::UnitOut,
    <A as CpuCircuit>::Inter,
);

pub enum Termination {
    /// Successfully halt
    Halt,
    /// Halt with error
    Error,
}

/// Pipeline simulator. A general CPU pipeline involves several pipeline registers
/// (flip-flops) and combinational logic circuits.
///
/// - Combinatorial logics: From `cur_state`, through `cur_unit_in`, `cur_inter`, `cur_unit_out`, to `nex_state`.
/// - Clock tick: from `nex_state`, controlled by stage input signals, to `cur_state`.
pub struct PipeSim<T: CpuArch> {
    pub(crate) circuit: PropCircuit<T>,
    pub(crate) cur_unit_in: T::UnitIn,
    pub(crate) cur_unit_out: T::UnitOut,
    pub(crate) cur_inter: T::Inter,
    pub(crate) cur_state: T::StageState,
    pub(crate) nex_state: T::StageState,
    pub(crate) units: T::Units,
    /// See [`PipeSim::is_terminate`].
    pub(crate) terminate: Option<Termination>,
    /// Whether to print the output to tty
    pub(crate) tty_out: bool,
}

impl<T: CpuArch> PipeSim<T> {
    // This function is called by hcl proc macro
    #[doc(hidden)]
    pub(crate) fn _is_terminate(&self) -> bool {
        self.terminate.is_some()
    }
    // This function is called by hcl proc macro
    #[doc(hidden)]
    pub(crate) fn _is_success(&self) -> bool {
        matches!(self.terminate, Some(Termination::Halt))
    }

    pub fn mem(&self) -> [u8; MEM_SIZE] {
        self.units.mem()
    }
}
