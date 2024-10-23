// This macro defines all pipeline registers in this architecture.
// In SEQ+ architecture, all stages are executed in a single cycle. But this
// time we need more information from the last cycle to compute the pc value
// compared to the SEQ architecture.
crate::define_stages! {
    /// The whole cycle is a single stage.
    SEQstage s {
        icode: u8 = NOP, valC: u64 = 0, valM: u64 = 0,
        valP: u64 = 0, cnd: bool = false
    }
}

sim_macro::hcl! {
#![hardware = crate::architectures::hardware_seq]
#![program_counter = pc]
#![termination = prog_term]
#![stage_alias(S => s)]

use Stat::*;

:==============================: Fetch Stage :================================:

// What address should instruction be fetched at
u64 pc = [
    // Call.  Use instruction constant
    S.icode == CALL : S.valC;
    // Taken branch.  Use instruction constant
    S.icode == JX && S.cnd : S.valC;
    // Completion of RET instruction.  Use value from stack
    S.icode == RET : S.valM;
    // Default: Use incremented PC
    true : S.valP;
];

@set_input(imem, {
    pc: pc
});

// Determine instruction code
u8 icode = [
    imem.error : NOP;
    true : imem.icode; // Default: get from instruction memory
];

// Determine instruction function
u8 ifun = [
    imem.error : 0; // set ifun to 0 if error
    true : imem.ifun;	// Default: get from instruction memory
];

bool instr_valid = icode in // CMOVX is the same as RRMOVQ
    { NOP, HALT, CMOVX, IRMOVQ, RMMOVQ, MRMOVQ,
    OPQ, JX, CALL, RET, PUSHQ, POPQ };

// Does fetched instruction require a regid byte?
bool need_regids =
    icode in { CMOVX, OPQ, PUSHQ, POPQ, IRMOVQ, RMMOVQ, MRMOVQ };

// Does fetched instruction require a constant word?
bool need_valC = icode in { IRMOVQ, RMMOVQ, MRMOVQ, JX, CALL };

@set_input(pc_inc, {
    need_valC: need_valC,
    need_regids: need_regids,
    old_pc: pc,
});

[u8; 9] align = imem.align;

@set_input(ialign, {
    align: align,
    need_regids: need_regids,
});

u64 valC = ialign.valC;
u64 valP = pc_inc.new_pc;

:=============================: Decode Stage :==============================:

// What register should be used as the A source?
u8 srcA = [
    icode in { CMOVX, RMMOVQ, OPQ, PUSHQ  } : ialign.rA;
    icode in { POPQ, RET } : RSP;
    true : RNONE; // Don't need register
];

// What register should be used as the B source?
u8 srcB = [
    icode in { OPQ, RMMOVQ, MRMOVQ } : ialign.rB;
    icode in { PUSHQ, POPQ, CALL, RET } : RSP;
    true : RNONE; // Don't need register
];

@set_input(reg_read, {
    srcA: srcA,
    srcB: srcB,
});

// What register should be used as the E destination?
u8 dstE = [
    icode in { CMOVX } && cnd : ialign.rB;
    icode in { IRMOVQ, OPQ} : ialign.rB;
    icode in { PUSHQ, POPQ, CALL, RET } : RSP;
    true : RNONE; // Don't write any register
];

// What register should be used as the M destination?
u8 dstM = [
    icode in { MRMOVQ, POPQ } : ialign.rA;
    true : RNONE; // Don't write any register
];

:==============================: Execute Stage :===============================:

// Select input A to ALU
u64 aluA = [
    icode in { CMOVX, OPQ } : reg_read.valA;
    icode in { IRMOVQ, RMMOVQ, MRMOVQ } : ialign.valC;
    icode in { CALL, PUSHQ } : NEG_8;
    icode in { RET, POPQ } : 8;
    // Other instructions don't need ALU
];

// Select input B to ALU
u64 aluB = [
    icode in { RMMOVQ, MRMOVQ, OPQ, CALL,
              PUSHQ, RET, POPQ } : reg_read.valB;
    icode in { CMOVX, IRMOVQ } : 0;
    // Other instructions don't need ALU
];

// Set the ALU function
u8 alufun = [
    icode == OPQ : ifun;
    true : ADD;
];

@set_input(alu, {
    a: aluA,
    b: aluB,
    fun: alufun,
});

// Should the condition codes be updated?
bool set_cc = icode in { OPQ };

u64 valE = alu.e;

@set_input(reg_cc, {
    a: aluA,
    b: aluB,
    e: valE,
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
    icode in { RMMOVQ, PUSHQ, CALL, MRMOVQ } : valE;
    icode in { POPQ, RET } : reg_read.valA;
    // Other instructions don't need address
];

// Select memory input data
u64 mem_data = [
    // Value from register
    icode in { RMMOVQ, PUSHQ } : reg_read.valA;
    // Return PC
    icode == CALL : valP;
    // Default: Don't write anything
];

@set_input(dmem, {
    read: mem_read,
    write: mem_write,
    addr: mem_addr,
    datain: mem_data,
});

u64 valM = dmem.dataout;

@set_input(reg_write, {
    dstE: dstE,
    dstM: dstM,
    valM: valM,
    valE: valE,
});

// Determine instruction status
Stat stat = [
    imem.error || dmem.error : Adr;
    !instr_valid : Ins;
    icode == HALT : Hlt;
    true : Aok;
];

bool prog_term = stat in { Hlt, Adr, Ins };

@set_stage(s, {
    valC: valC,
    valP: valP,
    icode: icode,
    cnd: cnd,
    valM: valM,
});

}

impl crate::framework::PipeSim<Arch> {
    fn print_state(&self) {}
}
