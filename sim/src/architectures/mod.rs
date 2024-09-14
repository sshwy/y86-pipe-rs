use hcl_macro::hcl;

use crate::{isa::BIN_SIZE, pipeline::Pipeline};

const NEG_8: u64 = -8i64 as u64;

hcl! {
#![hardware = crate::pipeline::hardware]

// "cur = pre" means: cur is the stage register in current cycle, pre is the
// stage register in the previous cycle. In other words, pre provides signals
// for the current cycle, and the output signals are stored in cur.
// e.g. M.vala is the value at the start of the cycle, i.m.vala is the value
// at the end of the cycle.
#![stage_alias(f = F, d = D, e = E, m = M, w = W)]

use crate::pipeline::Stat;
use crate::pipeline::Stat::*;

/////////////////////////// Fetch stage //////////////////////////

u64 f_pc = [
    // Mispredicted branch. Fetch at incremented PC
    #[tunnel(f_pc_fw_M_valA_MM)] M.icode == JX && !M.cnd => M.vala;
    // Completion of RET instruction
    #[tunnel(f_pc_fw_W_valM_WW)] W.icode == RET => W.valm;
    // Default: Use predicted value of PC
    #[tunnel(f_pc_F_predPC_FF)]  1 => F.pred_pc;
] -> (
    #[tunnel(PC_f_pc_FF)] i.pc_inc.old_pc,
    #[tunnel(IM_f_pc_FF)] i.imem.pc
);

// Determine icode of fetched instruction
u8 f_icode = [
    o.imem.error => NOP;
    1 => o.imem.icode;
] -> i.d.icode;

// Determine ifun
u8 f_ifun = [
    o.imem.error => 0xf; // FNONE;
    1 => o.imem.ifun;
] -> i.d.ifun;

[u8; 9] f_align = o.imem.align -> i.ialign.align;
u64 f_valc = #[tunnel(f_valC_D_valC_FF)] o.ialign.valc
    -> i.d.valc;
u64 f_valp = #[tunnel(f_valP_D_valP_FF)] o.pc_inc.new_pc
    -> i.d.valp;
u8 f_ra = o.ialign.ra -> i.d.ra;
u8 f_rb = o.ialign.rb -> i.d.rb;

// Is instruction valid?
bool instr_valid = f_icode in {NOP, HALT, CMOVX, IRMOVQ, RMMOVQ,
    MRMOVQ, OPQ, JX, CALL, RET, PUSHQ, POPQ};

// Determine status code for fetched instruction
Stat f_stat = [
    o.imem.error => Adr;
    !instr_valid => Ins;
    f_icode == HALT => Hlt;
    1 => Aok;
] -> i.d.stat;

// Does fetched instruction require a regid byte?
bool need_regids
    = f_icode in { CMOVX, OPQ, PUSHQ, POPQ, IRMOVQ, RMMOVQ, MRMOVQ}
    -> (i.pc_inc.need_regids, i.ialign.need_regids);

// Does fetched instruction require a constant word?
bool need_valc = f_icode in { IRMOVQ, RMMOVQ, MRMOVQ, JX, CALL }
    -> i.pc_inc.need_valc;

// Predict next value of PC
u64 f_pred_pc = [
    #[tunnel(f_predPC_f_valC_FF)] f_icode in { JX, CALL } => f_valc;
    #[tunnel(f_predPC_f_valP_FF)] 1 => f_valp;
] -> #[tunnel(f_predPC_FF)] i.f.pred_pc;

/////////////////// Decode and Write back stage ///////////////////

// What register should be used as the A source?
u8 d_srca = [
    D.icode in { CMOVX, RMMOVQ, OPQ, PUSHQ } => D.ra;
    D.icode in { POPQ, RET } => RSP;
    1 => RNONE;
] -> (i.reg_file.srca, i.e.srca);

// What register should be used as the B source?
u8 d_srcb = [
    D.icode in { OPQ, RMMOVQ, MRMOVQ } => D.rb;
    D.icode in { PUSHQ, POPQ, CALL, RET } => RSP;
    1 => RNONE;
] -> (i.reg_file.srcb, i.e.srcb);

// What register should be used as the E destination?
u8 d_dste = [
    D.icode in { CMOVX, IRMOVQ, OPQ } => D.rb;
    D.icode in { PUSHQ, POPQ, CALL, RET } => RSP;
    1 => RNONE;
] -> i.e.dste;

// What register should be used as the M destination?
u8 d_dstm = [
    D.icode in { MRMOVQ, POPQ } => D.ra;
    1 => RNONE;
] -> i.e.dstm;

u64 d_rvala = o.reg_file.vala;
u64 d_rvalb = o.reg_file.valb;

// What should be the A value?
// Forward into decode stage for valA
u64 d_vala = [
    #[tunnel(dec_D_valP_DD)]                // Use incremented PC
    D.icode in { CALL, JX } => D.valp;
    #[tunnel(fw_e_valE_EE)] #[tunnel(fw_e_valE_a_EE)] // Forward valE from execute
    d_srca == e_dste => e_vale;
    #[tunnel(fw_m_valM_MM)] #[tunnel(fw_m_valM_a_MM)] // Forward valM from memory
    d_srca == M.dstm => m_valm;
    #[tunnel(fw_M_valE_MM)] #[tunnel(fw_M_valE_a_MM)] // Forward valE from memory
    d_srca == M.dste => M.vale;
    #[tunnel(fw_W_valM_WW)] #[tunnel(fw_W_valM_a_WW)] // Forward valM from write back
    d_srca == W.dstm => W.valm;
    #[tunnel(fw_W_valE_WW)] #[tunnel(fw_W_valE_a_WW)] // Forward valE from write back
    d_srca == W.dste => W.vale;
    #[tunnel(dec_d_rvalA_DD)]               // Use value read from register file
    1 => d_rvala;
] -> #[tunnel(d_valA_DD)] i.e.vala;

u64 d_valb = [
    #[tunnel(fw_e_valE_EE)]  // Forward valE from execute
    d_srcb == e_dste => e_vale;
    #[tunnel(fw_m_valM_MM)]  // Forward valM from memory
    d_srcb == M.dstm => m_valm;
    #[tunnel(fw_M_valE_MM)]  // Forward valE from memory
    d_srcb == M.dste => M.vale;
    #[tunnel(fw_W_valM_WW)]  // Forward valM from write back
    d_srcb == W.dstm => W.valm;
    #[tunnel(fw_W_valE_WW)]  // Forward valE from write back
    d_srcb == W.dste => W.vale;
    #[tunnel(dec_d_rvalB_DD)] // Use value read from register file
    1 => d_rvalb;
] -> #[tunnel(d_valB_DD)] i.e.valb;

u64 d_valc = D.valc -> #[tunnel(d_valC)] i.e.valc;
u8 d_icode = D.icode -> i.e.icode;
u8 d_ifun = D.ifun -> i.e.ifun;
Stat d_stat = D.stat -> i.e.stat;

//////////////////////// Execute stage ////////////////////////////

// Select input A to ALU
u64 alua = [
    #[tunnel(aluA_valA_EE)] E.icode in { CMOVX, OPQ } => E.vala;
    #[tunnel(aluA_valC_EE)] E.icode in { IRMOVQ, RMMOVQ, MRMOVQ } => E.valc;
    E.icode in { CALL, PUSHQ } => NEG_8;
    E.icode in { RET, POPQ } => 8;
    // Other instructions don't need ALU, set to 0 for better debugging
    1 => 0;
] -> (#[tunnel(aluA_EE)] i.alu.a, i.cc.a);

// Select input B to ALU
u64 alub = [
    #[tunnel(aluB_valB_EE)]
    E.icode in { RMMOVQ, MRMOVQ, OPQ, CALL, PUSHQ, RET, POPQ } => E.valb;
    E.icode in { CMOVX, IRMOVQ } => 0;
    // Other instructions don't need ALU, set to 0 for better debugging
    1 => 0;
] -> (#[tunnel(aluB_EE)] i.alu.b, i.cc.b);

// Set the ALU function
u8 alufun = [
    E.icode == OPQ => E.ifun;
    1 => ADD;
] -> (i.alu.fun, i.cc.opfun);

Stat e_stat = E.stat -> i.m.stat;

// Should the condition codes be updated?
bool set_cc = E.icode == OPQ &&
    // State changes only during normal operation
    !(m_stat in { Adr, Ins, Hlt })
    && !(W.stat in { Adr, Ins, Hlt })
    -> i.cc.set_cc;

u64 e_vale = o.alu.e
    -> (#[tunnel(e_valE_EE)] i.m.vale, i.cc.e);

bool cc_sf = o.cc.sf -> i.cond.sf;
bool cc_of = o.cc.of -> i.cond.of;
bool cc_zf = o.cc.zf -> i.cond.zf;
u8 cond_fun = E.ifun -> i.cond.condfun; // jump fun

bool e_cnd = o.cond.cnd -> i.m.cnd;

// Generate valA in execute stage
u64 e_vala = E.vala -> i.m.vala;    // Pass valA through stage

// Set dstE to RNONE in event of not-taken conditional move
u8 e_dste = [
    E.icode == CMOVX && !e_cnd => RNONE;
    1 => E.dste;
] -> i.m.dste;

u8 e_dstm = E.dstm -> i.m.dstm;

u8 e_icode = E.icode -> i.m.icode;

//////////////////////// Memory stage /////////////////////////////

// Select memory address
u64 mem_addr = [
    #[tunnel(mem_addr_valE_MM)]
    M.icode in { RMMOVQ, PUSHQ, CALL, MRMOVQ } => M.vale;
    #[tunnel(mem_addr_valA_MM)]
    M.icode in { POPQ, RET } => M.vala;
    // Other instructions don't need address
] -> #[tunnel(DM_mem_addr_MM)] i.dmem.addr;

// Set read control signal
bool mem_read = M.icode in { MRMOVQ, POPQ, RET } -> i.dmem.read;

// Set write control signal
bool mem_write = M.icode in { RMMOVQ, PUSHQ, CALL } -> i.dmem.write;

u64 mem_datain = M.vala -> #[tunnel(DM_M_valA_MM)] i.dmem.datain;

// Update the status
Stat m_stat = [
    o.dmem.error => Adr;
    1 => M.stat;
] -> i.w.stat;

u8 m_icode = M.icode -> i.w.icode;

u64 m_valm = o.dmem.dataout -> #[tunnel(m_valM_MM)] i.w.valm;
u64 m_vale = M.vale -> #[tunnel(m_valE_MM)] i.w.vale;
u8 m_dste = M.dste -> i.w.dste;
u8 m_dstm = M.dstm -> i.w.dstm;

////////////////////// Write back stage ///////////////////////////

// Set E port register ID
u8 w_dste = W.dste -> i.reg_file.dste;

// Set E port value
u64 w_vale = W.vale -> i.reg_file.vale;

// Set M port register ID
u8 w_dstm = W.dstm -> i.reg_file.dstm;

// Set M port value
u64 w_valm = W.valm -> i.reg_file.valm;

// Update processor status (used for outside monitoring)
Stat prog_stat = [
    W.stat == Bub => Aok;
    1 => W.stat;
];

//////////////////// Pipeline Register Control /////////////////////

// Should I stall or inject a bubble into Pipeline Register F?
// At most one of these can be true.
bool f_bubble = false -> i.f.bubble;
bool f_stall =
    // Conditions for a load/use hazard
    E.icode in { MRMOVQ, POPQ } && E.dstm in { d_srca, d_srcb } ||
    // Stalling at fetch while ret passes through pipeline
    RET in {D.icode, E.icode, M.icode}
    -> i.f.stall;

// Should I stall or inject a bubble into Pipeline Register D?
// At most one of these can be true.
bool d_stall =
    // Conditions for a load/use hazard
    E.icode in { MRMOVQ, POPQ } && E.dstm in { d_srca, d_srcb }
    -> i.d.stall;

bool d_bubble =
    // Mispredicted branch
    (E.icode == JX && !e_cnd) ||
    // Stalling at fetch while ret passes through pipeline
    // but not condition for a load/use hazard
    !(E.icode in { MRMOVQ, POPQ } && E.dstm in { d_srca, d_srcb }) &&
      RET in {D.icode, E.icode, M.icode}
    -> i.d.bubble;

// Should I stall or inject a bubble into Pipeline Register E?
// At most one of these can be true.
bool e_stall = false -> i.e.stall;
bool e_bubble =
    // Mispredicted branch
    (E.icode == JX && !e_cnd) ||
    // Conditions for a load/use hazard
    E.icode in { MRMOVQ, POPQ } && E.dstm in { d_srca, d_srcb }
    -> i.e.bubble;

// Should I stall or inject a bubble into Pipeline Register M?
// At most one of these can be true.
bool m_stall = false -> i.m.stall;
// Start injecting bubbles as soon as exception passes through memory stage
bool m_bubble =
    m_stat in { Adr, Ins, Hlt } || W.stat in { Adr, Ins, Hlt }
    -> i.m.bubble;

// Should I stall or inject a bubble into Pipeline Register W?
bool w_stall = W.stat in { Adr, Ins, Hlt } -> i.w.stall;
bool w_bubble = false -> i.w.bubble;
}

impl Pipeline<Signals, Units, UnitInputSignal, UnitOutputSignal, IntermediateSignal> {
    pub fn init(bin: [u8; BIN_SIZE]) -> Self {
        let units = Units::init(bin);
        Self {
            circuit: Pipeline::build_circuit(),
            runtime_signals: Signals::default(),
            units,
            terminate: false,
        }
    }
    pub fn step(&mut self) -> (Signals, crate::propagate::Tracer) {
        println!("{:=^60}", " Run Cycle ");
        let (unit_out, tracer) = self.update();
        // for stage regitsers (compute for next):
        // - current info in this cycle: self.runtime_signals.1
        // - next cycle info: unit_out
        // for other devices (compute for current):
        // - current info in this cycle: unit_out
        // combinatorial logics:
        // - current self.runtime_signals.2
        let UnitOutputSignal {
            f,
            d,
            e,
            m,
            w,
            imem,
            ialign,
            pc_inc,
            reg_file,
            alu,
            cc,
            cond,
            dmem,
        } = unit_out;
        self.runtime_signals.1.imem = imem;
        self.runtime_signals.1.ialign = ialign;
        self.runtime_signals.1.pc_inc = pc_inc;
        self.runtime_signals.1.reg_file = reg_file;
        self.runtime_signals.1.alu = alu;
        self.runtime_signals.1.cc = cc;
        self.runtime_signals.1.cond = cond;
        self.runtime_signals.1.dmem = dmem;

        // processor state after this cycle
        let saved_state = self.runtime_signals.clone();
        self.print_state();

        let stat = self.runtime_signals.2.prog_stat;
        if stat != Stat::Aok && stat != Stat::Bub {
            self.terminate = true;
            eprintln!("terminate!");
        } else {
            // prepare for the next cycle
            self.runtime_signals.1.f = f;
            self.runtime_signals.1.d = d;
            self.runtime_signals.1.e = e;
            self.runtime_signals.1.m = m;
            self.runtime_signals.1.w = w;
        }

        (saved_state, tracer)
    }

    pub fn mem(&self) -> [u8; BIN_SIZE] {
        self.units.mem()
    }
}

#[rustfmt::skip]
mod nofmt {

use crate::isa::{inst_code, reg_code};
use ansi_term::Colour::{Red, Green};

use super::*;
impl Pipeline<Signals, Units, UnitInputSignal, UnitOutputSignal, IntermediateSignal> {
    // print state at the beginning of a cycle
    pub fn print_state(&self) {
        // For stage registers, outputs contains information for the following cycle
        let (i, o, c) = &self.runtime_signals;
        println!(

r#"{summary:-^60}
Stat    F {fstat:?}    D {dstat:?}    E {estat:?}    M {mstat:?}    W {wstat:?}
icode   f {ficode:6} D {dicode:6} E {eicode:6} M {micode:6} W {wicode:6}
Control F {fctrl:6} D {dctrl:6} E {ectrl:6} M {mctrl:6} W {wctrl:6}
f_pc {f_pc:#x} e_dste {e_dste} D_ra {d_ra} D_rb {d_rb}
{regs}

"#, 

summary = " Summary ",
fstat = Stat::Aok,
dstat = o.d.stat,
estat = o.e.stat,
mstat = o.m.stat,
wstat = o.w.stat,
// stage control at the end of the last cycle
fctrl = if i.f.bubble { Red.bold().paint("Bubble") } else if i.f.stall { Red.bold().paint("Stall ") } else { Green.paint("Normal") },
dctrl = if i.d.bubble { Red.bold().paint("Bubble") } else if i.d.stall { Red.bold().paint("Stall ") } else { Green.paint("Normal") },
ectrl = if i.e.bubble { Red.bold().paint("Bubble") } else if i.e.stall { Red.bold().paint("Stall ") } else { Green.paint("Normal") },
mctrl = if i.m.bubble { Red.bold().paint("Bubble") } else if i.m.stall { Red.bold().paint("Stall ") } else { Green.paint("Normal") },
wctrl = if i.w.bubble { Red.bold().paint("Bubble") } else if i.w.stall { Red.bold().paint("Stall ") } else { Green.paint("Normal") },
// ficode is actually computed value
ficode = inst_code::name_of(i.d.icode),
dicode = inst_code::name_of(o.d.icode),
eicode = inst_code::name_of(o.e.icode),
micode = inst_code::name_of(o.m.icode),
wicode = inst_code::name_of(o.w.icode),
f_pc = c.f_pc, e_dste = reg_code::name_of(c.e_dste),
d_ra = reg_code::name_of(o.d.ra), d_rb = reg_code::name_of(o.d.rb),
regs = self.units.print_reg()
);
    }
}

}

#[cfg(test)]
mod tests {
    use crate::{
        architectures::{IntermediateSignal, Signals, UnitInputSignal, UnitOutputSignal},
        asm::tests::RSUM_YS,
        assemble,
        pipeline::{hardware::Units, Pipeline},
    };

    #[test]
    fn test_hcl() {
        // let test_ys = include_str!("../../bubble.ys");
        let r = assemble(RSUM_YS, crate::AssembleOption::default()).unwrap();

        eprintln!("{}", r);
        let mut pipe: Pipeline<Signals, Units, UnitInputSignal, UnitOutputSignal, IntermediateSignal> = Pipeline::init(r.obj.binary.clone());
        // dbg!(&pipe.graph.nodes);
        while !pipe.is_terminate() {
            let _out = pipe.step();
            // mem_print(&pipe.mem());
            // eprintln!("{:?}\n", _out.1);
        }

        // mem_diff(&r.obj.binary, &pipe.mem());
        // mem_print(&pipe.mem());
        // eprintln!("{}", r);
        // eprintln!("{:?}", pipe.graph.levels);
    }
}
