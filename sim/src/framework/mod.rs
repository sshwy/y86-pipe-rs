//! To provide a flexible codebase for different CPU architectures, we give a
//! general CPU simulator framework.
mod propagate;

pub trait HardwareUnits {
    /// A set of hardware units should be initialized from a given memory.
    fn init(memory: [u8; MEM_SIZE]) -> Self;
    /// Get current memory data.
    fn mem(&self) -> [u8; MEM_SIZE];
    /// Return the registers and their values.
    ///
    /// (register_code, value)
    fn registers(&self) -> Vec<(u8, u64)>;
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
    /// behavior is undefined. This function should change the terminal state of the
    /// simulator if the simulation is terminated.
    fn propagate_signals(&mut self);

    /// Get the current program counter
    fn program_counter(&self) -> u64;
}

// here we use trait to collect the types
pub trait CpuCircuit {
    type UnitIn: Default;
    type UnitOut: Default;
    type Inter: Default;
    type StageState: Default;
}

pub trait CpuArch: CpuCircuit + Sized {
    type Units: HardwareUnits;
    fn build_circuit() -> PropCircuit<Self>;
}

pub type Signals<A> = (
    <A as CpuCircuit>::UnitIn,
    <A as CpuCircuit>::UnitOut,
    <A as CpuCircuit>::Inter,
);

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
    pub(crate) terminate: bool,
    /// Whether to print the output to tty
    pub(crate) tty_out: bool,
    pub(crate) cycle_count: u64,
}

impl<T: CpuArch> PipeSim<T> {
    /// Initialize the simulator with given memory
    ///
    /// tty_out: whether to print rich-text information
    pub fn new(memory: [u8; crate::framework::MEM_SIZE], tty_out: bool) -> Self {
        Self {
            circuit: T::build_circuit(),
            cur_inter: T::Inter::default(),
            cur_unit_in: T::UnitIn::default(),
            cur_unit_out: T::UnitOut::default(),
            cur_state: T::StageState::default(),
            nex_state: T::StageState::default(),
            units: T::Units::init(memory),
            terminate: false,
            tty_out,
            cycle_count: 0,
        }
    }

    /// Whether the simulation is terminated
    pub fn is_terminate(&self) -> bool {
        self.terminate
    }
    pub fn cycle_count(&self) -> u64 {
        self.cycle_count
    }
    pub fn mem(&self) -> [u8; MEM_SIZE] {
        self.units.mem()
    }
    /// Get the registers and their values
    pub fn registers(&self) -> Vec<(u8, u64)> {
        self.units.registers()
    }
}
