use crate::architectures::hardware_seq::Stat::*;

crate::define_stages! {
    /// The whole cycle is a single stage.
    SEQstage s {
        icode: u8 = NOP, valc: u64 = 0, valm: u64 = 0,
        valp: u64 = 0, cnd: bool = false
    }
}

sim_macro::hcl! {
#![hardware = crate::architectures::hardware_seq]
#![program_counter = pc]
#![termination = prog_term]
#![stage_alias(S => s)]

:==============================: Fetch Stage :================================:

// What address should instruction be fetched at
u64 pc = [
    // Call.  Use instruction constant
    S.icode == CALL : S.valc;
    // Taken branch.  Use instruction constant
    S.icode == JX && S.cnd : S.valc;
    // Completion of RET instruction.  Use value from stack
    S.icode == RET : S.valm;
    // Default: Use incremented PC
    true : S.valp;
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

u64 valc = ialign.valc;
u64 valp = pc_inc.new_pc;

:=============================: Decode Stage :==============================:

// What register should be used as the A source?
u8 srca = [
    icode in { CMOVX, RMMOVQ, OPQ, PUSHQ  } : ialign.ra;
    icode in { POPQ, RET } : RSP;
    true : RNONE; // Don't need register
];

// What register should be used as the B source?
u8 srcb = [
    icode in { OPQ, RMMOVQ, MRMOVQ } : ialign.rb;
    icode in { PUSHQ, POPQ, CALL, RET } : RSP;
    true : RNONE; // Don't need register
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
    true : RNONE; // Don't write any register
];

// What register should be used as the M destination?
u8 dstm = [
    icode in { MRMOVQ, POPQ } : ialign.ra;
    true : RNONE; // Don't write any register
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
    true : ADD;
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
    true : Aok;
];

bool prog_term = stat in { Hlt, Adr, Ins };

@set_stage(s, {
    valc: valc,
    valp: valp,
    icode: icode,
    cnd: cnd,
    valm: valm,
});

}

impl crate::framework::PipeSim<Arch> {
    fn print_state(&self) {}
}
