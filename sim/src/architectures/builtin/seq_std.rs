use crate::architectures::hardware_seq::Stat::*;

crate::define_stages! {
    /// The whole cycle is a single stage.
    SEQstage s { pc: u64 = 0 }
}

sim_macro::hcl! {

// Specify the CPU hardware devices set.
// This will imports all items from the hardware module.
#![hardware = crate::architectures::hardware_seq]

// Specify the program counter by an intermediate signal. This value is read by
// debugger. Conventionally, when we create a breakpoint at the line of code, the
// debugger seems to stop before executing the line of code. But in this simulator,
// The breakpoint take effects when the current cycle is executed (so the value of pc
// is calculated) and before the next cycle enters.
//
// Changing this value to other signals makes no difference to the simulation.
// But it affects the behavior of the debugger.
#![program_counter = pc]

// Specify a boolean intermediate signal to indicate whether the program should
// be terminated.
#![termination = prog_term]

// This attribute defines the identifiers for pipeline registers. For "F => f", the
// identifier `f` is the short name in [`crate::define_stages`], and `F` can be
// arbitrarily chosen.
//
// e.g. S.pc is the value at the start of the cycle (you should treat it as
// read-only), s.pc is the value at the end of the cycle (you should assign to it).
#![stage_alias(S => s)]

:==============================: Fetch Stage :================================:

u64 pc = S.pc;

@set_input(imem, {
    pc: pc
});

// Determine instruction code
u8 icode = [
    imem.error : NOP;
    1 : imem.icode; // Default: get from instruction memory
];

// Determine instruction function
u8 ifun = [
    imem.error : 0; // set ifun to 0 if error
    1 : imem.ifun;	// Default: get from instruction memory
];

bool instr_valid = icode in // CMOVX is the same as RRMOVQ
    { NOP, HALT, CMOVX, IRMOVQ, RMMOVQ, MRMOVQ,
    OPQ, JX, CALL, RET, PUSHQ, POPQ };

// Does fetched instruction require a regid byte?
bool need_regids =
    icode in { CMOVX, OPQ, PUSHQ, POPQ, IRMOVQ, RMMOVQ, MRMOVQ };

// Does fetched instruction require a constant word?
bool need_valc = icode in { IRMOVQ, RMMOVQ, MRMOVQ, JX, CALL };

@set_input(pc_inc, {
    need_valc: need_valc,
    need_regids: need_regids,
    old_pc: pc,
});

[u8; 9] align = imem.align;

@set_input(ialign, {
    align: align,
    need_regids: need_regids,
});

u64 valp = pc_inc.new_pc;

:=============================: Decode Stage :==============================:

// What register should be used as the A source?
u8 srca = [
    icode in { CMOVX, RMMOVQ, OPQ, PUSHQ  } : ialign.ra;
    icode in { POPQ, RET } : RSP;
    1 : RNONE; // Don't need register
];

// What register should be used as the B source?
u8 srcb = [
    icode in { OPQ, RMMOVQ, MRMOVQ } : ialign.rb;
    icode in { PUSHQ, POPQ, CALL, RET } : RSP;
    1 : RNONE; // Don't need register
];

@set_input(reg_read, {
    srca: srca,
    srcb: srcb,
});

// What register should be used as the E destination?
u8 dste = [
    icode in { CMOVX } && cnd : ialign.rb;
    icode in { IRMOVQ, OPQ} : ialign.rb;
    icode in { PUSHQ, POPQ, CALL, RET } : RSP;
    1 : RNONE; // Don't write any register
];

// What register should be used as the M destination?
u8 dstm = [
    icode in { MRMOVQ, POPQ } : ialign.ra;
    1 : RNONE; // Don't write any register
];

:==============================: Execute Stage :===============================:

// Select input A to ALU
u64 alua = [
    icode in { CMOVX, OPQ } : reg_read.vala;
    icode in { IRMOVQ, RMMOVQ, MRMOVQ } : ialign.valc;
    icode in { CALL, PUSHQ } : NEG_8;
    icode in { RET, POPQ } : 8;
    // Other instructions don't need ALU
];

// Select input B to ALU
u64 alub = [
    icode in { RMMOVQ, MRMOVQ, OPQ, CALL,
              PUSHQ, RET, POPQ } : reg_read.valb;
    icode in { CMOVX, IRMOVQ } : 0;
    // Other instructions don't need ALU
];

// Set the ALU function
u8 alufun = [
    icode == OPQ : ifun;
    1 : ADD;
];

@set_input(alu, {
    a: alua,
    b: alub,
    fun: alufun,
});

// Should the condition codes be updated?
bool set_cc = icode in { OPQ };

u64 vale = alu.e;

@set_input(reg_cc, {
    a: alua,
    b: alub,
    e: vale,
    opfun: alufun,
    set_cc: set_cc,
});

ConditionCode cc = reg_cc.cc;

@set_input(cond, {
    cc: cc,
    condfun: ifun,
});

bool cnd = cond.cnd;

:===============================: Memory Stage :===============================:

// Set read control signal
bool mem_read = icode in { MRMOVQ, POPQ, RET };

// Set write control signal
bool mem_write = icode in { RMMOVQ, PUSHQ, CALL };

// Select memory address
u64 mem_addr = [
    icode in { RMMOVQ, PUSHQ, CALL, MRMOVQ } : vale;
    icode in { POPQ, RET } : reg_read.vala;
    // Other instructions don't need address
];

// Select memory input data
u64 mem_data = [
    // Value from register
    icode in { RMMOVQ, PUSHQ } : reg_read.vala;
    // Return PC
    icode == CALL : valp;
    // Default: Don't write anything
];

@set_input(dmem, {
    read: mem_read,
    write: mem_write,
    addr: mem_addr,
    datain: mem_data,
});

u64 valm = dmem.dataout;

@set_input(reg_write, {
    dste: dste,
    dstm: dstm,
    valm: valm,
    vale: vale,
});

// Determine instruction status
Stat stat = [
    imem.error || dmem.error : Adr;
    !instr_valid : Ins;
    icode == HALT : Hlt;
    1 : Aok;
];

bool prog_term = stat in { Hlt, Adr, Ins };

:==========================: Program Counter Update :==========================:

// What address should instruction be fetched at

u64 new_pc = [
    // Call.  Use instruction constant
    icode == CALL : ialign.valc;
    // Taken branch.  Use instruction constant
    icode == JX && cnd : ialign.valc;
    // Completion of RET instruction.  Use value from stack
    icode == RET : valm;
    // Default: Use incremented PC
    1 : valp;
];

@set_stage(s, {
    pc: new_pc,
});
}

impl crate::framework::PipeSim<Arch> {
    fn print_state(&self) {}
}
