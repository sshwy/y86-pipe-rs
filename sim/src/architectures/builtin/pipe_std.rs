use crate::architectures::hardware_pipe::Stat::*;

crate::define_stages! {
    /// Fetch stage registers.
    /// note that it's not possible to bubble (see hcl)
    Fstage f {
        pred_pc: u64 = 0
    }
    Dstage d {
        stat: Stat = Bub, icode: u8 = NOP, ifun: u8 = 0,
        ra: u8 = RNONE, rb: u8 = RNONE, valc: u64 = 0, valp: u64 = 0
    }
    Estage e {
        stat: Stat = Bub, icode: u8 = NOP, ifun: u8 = 0,
        vala: u64 = 0, valb: u64 = 0, valc: u64 = 0, dste: u8 = RNONE,
        dstm: u8 = RNONE, srca: u8 = RNONE, srcb: u8 = RNONE
    }
    /// Memory Access Stage
    Mstage m {
        stat: Stat = Bub, icode: u8 = NOP, cnd: bool = false,
        vale: u64 = 0, vala: u64 = 0, dste: u8 = RNONE, dstm: u8 = RNONE
    }
    Wstage w {
        stat: Stat = Bub, icode: u8 = NOP, vale: u64 = 0,
        valm: u64 = 0, dste: u8 = RNONE, dstm: u8 = RNONE
    }
}

sim_macro::hcl! {

// Specify the CPU hardware devices set.
// This will imports all items from the hardware module.
#![hardware = crate::architectures::hardware_pipe]

// Specify the program counter by an intermediate signal. This value is read by
// debugger. Conventionally, when we create a breakpoint at the line of code, the
// debugger seems to stop before executing the line of code. But in this simulator,
// The breakpoint take effects when the current cycle is executed (so the value of pc
// is calculated) and before the next cycle enters.
//
// Changing this value to other signals makes no difference to the simulation.
// But it affects the behavior of the debugger.
#![program_counter = f_pc]

// Specify a boolean intermediate signal to indicate whether the program should
// be terminated.
#![termination = prog_term]

// This attribute defines the identifiers for pipeline registers. For "F => f", the
// identifier `f` is the short name in [`crate::define_stages`], and `F` can be
// arbitrarily chosen.
//
// e.g. M.vala is the value at the start of the cycle (you should treat it as
// read-only), m.vala is the value at the end of the cycle (you should assign to it).
#![stage_alias(F => f, D => d, E => e, M => m, W => w)]

// You can use `:====: title :====:` to declare a section. This helps to organize
// your code and the information displayed by debugger. It makes no difference in
// the simulation. That means it does not alter the evaluation order of CPU cycle.
:==============================: Fetch Stage :================================:

// What address should instruction be fetched at
u64 f_pc = [
    // Mispredicted branch. Fetch at incremented PC
    M.icode == JX && !M.cnd : M.vala;
    // Completion of RET instruction
    W.icode == RET : W.valm;
    // Default: Use predicted value of PC (default to 0)
     1 : F.pred_pc;
];

@set_input(imem, {
    pc: f_pc
});

// Determine icode of fetched instruction
u8 f_icode = [
    imem.error : NOP;
    1 : imem.icode;
];

// Determine ifun
u8 f_ifun = [
    imem.error : 0xf; // FNONE;
    1 : imem.ifun;
];


// Is instruction valid?
bool instr_valid = f_icode in { NOP, HALT, CMOVX, IRMOVQ, RMMOVQ,
    MRMOVQ, OPQ, JX, CALL, RET, PUSHQ, POPQ };

// Determine status code for fetched instruction
Stat f_stat = [
    imem.error : Adr;
    !instr_valid : Ins;
    f_icode == HALT : Hlt;
    1 : Aok;
];

// Does fetched instruction require a regid byte?
bool need_regids
    = f_icode in { CMOVX, OPQ, PUSHQ, POPQ, IRMOVQ, RMMOVQ, MRMOVQ };

// Does fetched instruction require a constant word?
bool need_valc = f_icode in { IRMOVQ, RMMOVQ, MRMOVQ, JX, CALL };

@set_input(pc_inc, {
    need_valc: need_valc,
    need_regids: need_regids,
    old_pc: f_pc,
});

u64 f_valp =  pc_inc.new_pc;

[u8; 9] f_align = imem.align;

@set_input(ialign, {
    align: f_align,
    need_regids: need_regids,
});

u64 f_valc =  ialign.valc;
u8 f_ra = ialign.ra;
u8 f_rb = ialign.rb;

// Predict next value of PC
u64 f_pred_pc = [
     f_icode in { JX, CALL } : f_valc;
     1 : f_valp;
];

@set_stage(f, {
    pred_pc: f_pred_pc,
});

@set_stage(d, {
    icode: f_icode,
    ifun: f_ifun,
    stat: f_stat,
    valc: f_valc,
    valp: f_valp,
    ra: f_ra,
    rb: f_rb,
});

:=======================: Decode and Write Back Stage :========================:

// What register should be used as the A source?
u8 d_srca = [
    D.icode in { CMOVX, RMMOVQ, OPQ, PUSHQ } : D.ra;
    D.icode in { POPQ, RET } : RSP;
    1 : RNONE; // Don't need register
];

// What register should be used as the B source?
u8 d_srcb = [
    D.icode in { OPQ, RMMOVQ, MRMOVQ } : D.rb;
    D.icode in { PUSHQ, POPQ, CALL, RET } : RSP;
    1 : RNONE; // Don't need register
];

// What register should be used as the E destination?
u8 d_dste = [
    D.icode in { CMOVX, IRMOVQ, OPQ } : D.rb;
    D.icode in { PUSHQ, POPQ, CALL, RET } : RSP;
    1 : RNONE; // Don't write any register
];

// What register should be used as the M destination?
u8 d_dstm = [
    D.icode in { MRMOVQ, POPQ } : D.ra;
    1 : RNONE; // Don't write any register
];

@set_input(reg_file, {
    srca: d_srca,
    srcb: d_srcb,
    dste: w_dste,
    dstm: w_dstm,
    valm: w_valm,
    vale: w_vale,
});

u64 d_rvala = reg_file.vala;
u64 d_rvalb = reg_file.valb;

// What should be the A value?
// Forward into decode stage for valA
u64 d_vala = [
    D.icode in { CALL, JX } : D.valp; // Use incremented PC
    d_srca == e_dste : e_vale; // Forward valE from execute
    d_srca == M.dstm : m_valm; // Forward valM from memory
    d_srca == M.dste : M.vale; // Forward valE from memory
    d_srca == W.dstm : W.valm; // Forward valM from write back
    d_srca == W.dste : W.vale; // Forward valE from write back
    1 : d_rvala; // Use value read from register file
];

u64 d_valb = [
    d_srcb == e_dste : e_vale; // Forward valE from execute
    d_srcb == M.dstm : m_valm; // Forward valM from memory
    d_srcb == M.dste : M.vale; // Forward valE from memory
    d_srcb == W.dstm : W.valm; // Forward valM from write back
    d_srcb == W.dste : W.vale; // Forward valE from write back
    1 : d_rvalb; // Use value read from register file
];

u64 d_valc = D.valc;
u8 d_icode = D.icode;
u8 d_ifun = D.ifun;
Stat d_stat = D.stat;

@set_stage(e, {
    icode: d_icode,
    ifun: d_ifun,
    stat: d_stat,
    valc: d_valc,
    srca: d_srca,
    srcb: d_srcb,
    vala: d_vala,
    valb: d_valb,
    dste: d_dste,
    dstm: d_dstm,
});

:==============================: Execute Stage :===============================:

// Select input A to ALU
u64 alua = [
    E.icode in { CMOVX, OPQ } : E.vala;
    E.icode in { IRMOVQ, RMMOVQ, MRMOVQ } : E.valc;
    E.icode in { CALL, PUSHQ } : NEG_8;
    E.icode in { RET, POPQ } : 8;
    1 : 0; // Other instructions don't need ALU
];

// Select input B to ALU
u64 alub = [

    E.icode in { RMMOVQ, MRMOVQ, OPQ, CALL, PUSHQ, RET, POPQ } : E.valb;
    E.icode in { CMOVX, IRMOVQ } : 0;
    1 : 0; // Other instructions don't need ALU
];

// Set the ALU function
u8 alufun = [
    E.icode == OPQ : E.ifun;
    1 : ADD;
];

@set_input(alu, {
    a: alua,
    b: alub,
    fun: alufun,
});

// Should the condition codes be updated?
bool set_cc = E.icode == OPQ &&
    // State changes only during normal operation
    !(m_stat in { Adr, Ins, Hlt }) && !(W.stat in { Adr, Ins, Hlt });

u64 e_vale = alu.e;

@set_input(reg_cc, {
    a: alua,
    b: alub,
    e: e_vale,
    opfun: alufun,
    set_cc: set_cc,
});


ConditionCode cc = reg_cc.cc;
u8 e_ifun = E.ifun;

@set_input(cond, {
    cc: cc,
    condfun: e_ifun,
});

bool e_cnd = cond.cnd;

// Generate valA in execute stage
u64 e_vala = E.vala;    // Pass valA through stage

// Set dstE to RNONE in event of not-taken conditional move
u8 e_dste = [
    E.icode == CMOVX && !e_cnd : RNONE;
    1 : E.dste;
];

u8 e_dstm = E.dstm;
u8 e_icode = E.icode;
Stat e_stat = E.stat;

@set_stage(m, {
    stat: e_stat,
    dstm: e_dstm,
    icode: e_icode,
    dste: e_dste,
    cnd: e_cnd,
    vale: e_vale,
    vala: e_vala,
});

:===============================: Memory Stage :===============================:

// Select memory address
u64 mem_addr = [
    M.icode in { RMMOVQ, PUSHQ, CALL, MRMOVQ } : M.vale;
    M.icode in { POPQ, RET } : M.vala;
    // Other instructions don't need address
];

// Set read control signal
bool mem_read = M.icode in { MRMOVQ, POPQ, RET };

// Set write control signal
bool mem_write = M.icode in { RMMOVQ, PUSHQ, CALL };

u64 mem_data = M.vala;

@set_input(dmem, {
    read: mem_read,
    write: mem_write,
    addr: mem_addr,
    datain: mem_data,
});

// Update the status
Stat m_stat = [
    dmem.error : Adr;
    1 : M.stat;
];

u8 m_icode = M.icode;

u64 m_valm = dmem.dataout;
u64 m_vale = M.vale;
u8 m_dste = M.dste;
u8 m_dstm = M.dstm;

@set_stage(w, {
    stat: m_stat,
    icode: m_icode,
    vale: m_vale,
    valm: m_valm,
    dste: m_dste,
    dstm: m_dstm,
});

:=============================: Write Back Stage :=============================:

// Set E port register ID
u8 w_dste = W.dste;

// Set E port value
u64 w_vale = W.vale;

// Set M port register ID
u8 w_dstm = W.dstm;

// Set M port value
u64 w_valm = W.valm;

// Update processor status (used for outside monitoring)
Stat prog_stat = [
    W.stat == Bub : Aok;
    1 : W.stat;
];

bool prog_term = [
    prog_stat in { Aok, Bub } : false;
    1 : true
];

:========================: Pipeline Register Control :=========================:

// Should I stall or inject a bubble into Pipeline Register F?
// At most one of these can be true.
bool f_bubble = false;
bool f_stall =
    // Conditions for a load/use hazard
    E.icode in { MRMOVQ, POPQ } && E.dstm in { d_srca, d_srcb } ||
    // Stalling at fetch while ret passes through pipeline
    RET in {D.icode, E.icode, M.icode};

@set_stage(f, {
    bubble: f_bubble,
    stall: f_stall,
});

// Should I stall or inject a bubble into Pipeline Register D?
// At most one of these can be true.
bool d_stall =
    // Conditions for a load/use hazard
    E.icode in { MRMOVQ, POPQ } && E.dstm in { d_srca, d_srcb };

bool d_bubble =
    // Mispredicted branch
    (E.icode == JX && !e_cnd) ||
    // Stalling at fetch while ret passes through pipeline
    // but not condition for a load/use hazard
    !(E.icode in { MRMOVQ, POPQ } && E.dstm in { d_srca, d_srcb }) &&
      RET in {D.icode, E.icode, M.icode};

@set_stage(d, {
    stall: d_stall,
    bubble: d_bubble,
});

// Should I stall or inject a bubble into Pipeline Register E?
// At most one of these can be true.
bool e_stall = false;
bool e_bubble =
    // Mispredicted branch
    (E.icode == JX && !e_cnd) ||
    // Conditions for a load/use hazard
    E.icode in { MRMOVQ, POPQ } && E.dstm in { d_srca, d_srcb };

@set_stage(e, {
    stall: e_stall,
    bubble: e_bubble,
});

// Should I stall or inject a bubble into Pipeline Register M?
// At most one of these can be true.
bool m_stall = false;
// Start injecting bubbles as soon as exception passes through memory stage
bool m_bubble =
    m_stat in { Adr, Ins, Hlt } || W.stat in { Adr, Ins, Hlt };

@set_stage(m, {
    stall: m_stall,
    bubble: m_bubble,
});

// Should I stall or inject a bubble into Pipeline Register W?
bool w_stall = W.stat in { Adr, Ins, Hlt };
bool w_bubble = false;

@set_stage(w, {
    stall: w_stall,
    bubble: w_bubble,
});
}

mod nofmt {
    use crate::framework::PipeSim;

    use crate::isa::reg_code;
    use crate::utils::{format_ctrl, format_icode};

    use super::*;
    impl PipeSim<Arch> {
        // print state at the beginning of a cycle
        pub fn print_state(&self) {
            // For stage registers, outputs contains information for the following cycle
            let c = &self.cur_inter;

            #[allow(non_snake_case)]
            let PipeRegs {
                f: _,
                d: D,
                e: E,
                m: M,
                w: W,
            } = &self.cur_state;
            let PipeRegs { f, d, e, m, w } = &self.nex_state;

            println!(
                r#"Stat    F {fstat}    D {dstat}    E {estat}    M {mstat}    W {wstat}
icode   f {ficode} D {dicode} E {eicode} M {micode} W {wicode}
Control F {fctrl:6} D {dctrl:6} E {ectrl:6} M {mctrl:6} W {wctrl:6}
e_dste {e_dste} D_ra {d_ra} D_rb {d_rb}"#,
                fstat = Aok,
                dstat = D.stat,
                estat = E.stat,
                mstat = M.stat,
                wstat = W.stat,
                // stage control at the end of last cycle
                // e.g. dctrl is computed in fetch stage. if dctrl is bubble,
                // then in the next cycle, D.icode will be NOP.
                // e. Controls are applied between cycles.
                fctrl = format_ctrl(f.bubble, f.stall),
                dctrl = format_ctrl(d.bubble, d.stall),
                ectrl = format_ctrl(e.bubble, e.stall),
                mctrl = format_ctrl(m.bubble, m.stall),
                wctrl = format_ctrl(w.bubble, w.stall),
                // ficode is actually computed value
                ficode = format_icode(d.icode),
                dicode = format_icode(D.icode),
                eicode = format_icode(E.icode),
                micode = format_icode(M.icode),
                wicode = format_icode(W.icode),
                e_dste = reg_code::name_of(c.e_dste),
                d_ra = reg_code::name_of(D.ra),
                d_rb = reg_code::name_of(D.rb),
            );
        }
    }
}
