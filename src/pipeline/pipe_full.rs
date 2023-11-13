use crate::hcl;

hcl! {

@hardware crate::pipeline::hardware;
@devinput i;     // input of a device, or next stage state (fdemw)
@devoutput o;    // output of a device, or current stage state (FDEMW)
@intermediate c; // intermediate value
@abbr F D E M W

@use crate::pipeline::Stat;

/////////////////// Fetch stage ///////////////////

f_pc u64 = [
    // Mispredicted branch.  Fetch at incremented PC
    M.icode == JX && !M.cnd => M.vala;
    // Completion of RET instruction
    W.icode == RET => W.valm;
    // Default: Use predicted value of PC
    1 => F.pred_pc;
] => i.pc_inc.old_pc
    => i.imem.pc;

// Determine icode of fetched instruction
f_icode u8 = [
    o.imem.error => NOP;
    1 => o.imem.icode;
] => i.d.icode;

// Determine ifun
f_ifun u8 = [
    o.imem.error => 0xf; // FNONE;
    1 => o.imem.ifun;
] => i.d.ifun;

f_align [u8; 9] := o.imem.align => i.ialign.align;
f_valc u64 := o.ialign.valc => i.d.valc;
f_valp u64 := o.pc_inc.new_pc => i.d.valp;
f_ra u8 := o.ialign.ra => i.d.ra;
f_rb u8 := o.ialign.rb => i.d.rb;

// Is instruction valid?
instr_valid bool := mtc(c.f_icode, [NOP, HALT, CMOVX, IRMOVQ, RMMOVQ,
    MRMOVQ, OPQ, JX, CALL, RET, PUSHQ, POPQ]);

// Determine status code for fetched instruction
f_stat Stat = [
    o.imem.error => Stat::Adr;
    !c.instr_valid => Stat::Ins;
    c.f_icode == HALT => Stat::Hlt;
    1 => Stat::Aok;
] => i.d.stat;

// Does fetched instruction require a regid byte?
need_regids bool
    := mtc(c.f_icode, [ CMOVX, OPQ, PUSHQ, POPQ, IRMOVQ, RMMOVQ, MRMOVQ])
    => i.pc_inc.need_regids
    => i.ialign.need_regids;

// Does fetched instruction require a constant word?
need_valc bool
    := mtc(c.f_icode, [ IRMOVQ, RMMOVQ, MRMOVQ, JX, CALL])
    => i.pc_inc.need_valc;

// Predict next value of PC
f_pred_pc u64 = [
    mtc(c.f_icode, [JX, CALL]) => c.f_valc;
    1 => c.f_valp;
] => i.f.pred_pc;

/////////////////// Decode and Write back stage ///////////////////

// What register should be used as the A source?
d_srca u8 = [
    mtc(D.icode, [CMOVX, RMMOVQ, OPQ, PUSHQ]) => D.ra;
    mtc(D.icode, [ POPQ, RET ]) => RSP;
    1 => RNONE;
] => i.reg_file.srca
  => i.e.srca;

// What register should be used as the B source?
d_srcb u8 = [
    mtc(D.icode, [ OPQ, RMMOVQ, MRMOVQ ]) => D.rb;
    mtc(D.icode, [ PUSHQ, POPQ, CALL, RET ]) => RSP;
    1 => RNONE;
] => i.reg_file.srcb
  => i.e.srcb;

// What register should be used as the E destination?
d_dste u8 = [
    mtc(D.icode, [ CMOVX, IRMOVQ, OPQ ]) => D.rb;
    mtc(D.icode, [ PUSHQ, POPQ, CALL, RET ]) => RSP;
    1 => RNONE;
] => i.e.dste;

// What register should be used as the M destination?
d_dstm u8 = [
    mtc(D.icode, [ MRMOVQ, POPQ ]) => D.ra;
    1 => RNONE;
] => i.e.dstm;

d_rvala u64 := o.reg_file.vala;
d_rvalb u64 := o.reg_file.valb;

// What should be the A value?
// Forward into decode stage for valA
d_vala u64 = [
    mtc(D.icode, [CALL, JX]) => D.valp;  // Use incremented PC
    c.d_srca == c.e_dste => c.e_vale;    // Forward valE from execute
    c.d_srca == M.dstm => c.m_valm;      // Forward valM from memory
    c.d_srca == M.dste => M.vale;        // Forward valE from memory
    c.d_srca == W.dstm => W.valm;        // Forward valM from write back
    c.d_srca == W.dste => W.vale;        // Forward valE from write back
    1 => c.d_rvala;                      // Use value read from register file
] => i.e.vala;

d_valb u64 = [
    c.d_srcb == c.e_dste => c.e_vale;    // Forward valE from execute
    c.d_srcb == M.dstm => c.m_valm;      // Forward valM from memory
    c.d_srcb == M.dste => M.vale;        // Forward valE from memory
    c.d_srcb == W.dstm => W.valm;        // Forward valM from write back
    c.d_srcb == W.dste => W.vale;        // Forward valE from write back
    1 => c.d_rvalb;                      // Use value read from register file
] => i.e.valb;

d_valc u64 := D.valc => i.e.valc;
d_icode u8 := D.icode => i.e.icode;
d_ifun u8 := D.ifun => i.e.ifun;
d_stat Stat := D.stat => i.e.stat;

/////////////////// Execute stage ///////////////////

// Select input A to ALU
alua u64 = [
    mtc(E.icode, [CMOVX, OPQ ]) => E.vala;
    mtc(E.icode, [IRMOVQ, RMMOVQ, MRMOVQ ]) => E.valc;
    mtc(E.icode, [CALL, PUSHQ ]) => -8i64 as u64;
    mtc(E.icode, [RET, POPQ ]) => 8;
    // Other instructions don't need ALU
] => i.alu.a
  => i.cc.a;

// Select input B to ALU
alub u64 = [
    mtc(E.icode, [RMMOVQ, MRMOVQ, OPQ, CALL, PUSHQ, RET, POPQ]) => E.valb;
    mtc(E.icode, [CMOVX, IRMOVQ]) => 0;
    // Other instructions don't need ALU
] => i.alu.b
  => i.cc.b;

// Set the ALU function
alufun u8 = [
    E.icode == OPQ => E.ifun;
    1 => ADD;
] => i.alu.fun
  => i.cc.opfun;

e_stat Stat := E.stat => i.m.stat;

// Should the condition codes be updated?
set_cc bool := E.icode == OPQ &&
    // State changes only during normal operation
    !mtc(c.m_stat, [Stat::Adr, Stat::Ins, Stat::Hlt])
    && !mtc(W.stat, [Stat::Adr, Stat::Ins, Stat::Hlt])
    => i.cc.set_cc;

e_vale u64 := o.alu.e
    => i.m.vale
    => i.cc.e;

cc_sf bool := o.cc.sf => i.cond.sf;
cc_of bool := o.cc.of => i.cond.of;
cc_zf bool := o.cc.zf => i.cond.zf;
cond_fun u8 := E.ifun => i.cond.condfun; // jump fun

e_cnd bool := o.cond.cnd => i.m.cnd;

// Generate valA in execute stage
e_vala u64 := E.vala => i.m.vala;    // Pass valA through stage

// Set dstE to RNONE in event of not-taken conditional move
e_dste u8 = [
    E.icode == CMOVX && !c.e_cnd => RNONE;
    1 => E.dste;
] => i.m.dste;

e_dstm u8 := E.dstm => i.m.dstm;

e_icode u8 := E.icode => i.m.icode;

/////////////////// Memory stage ///////////////////

// Select memory address
mem_addr u64 = [
    mtc(M.icode, [RMMOVQ, PUSHQ, CALL, MRMOVQ]) => M.vale;
    mtc(M.icode, [POPQ, RET]) => M.vala;
    // Other instructions don't need address
] => i.dmem.addr;

// Set read control signal
mem_read bool := mtc(M.icode, [MRMOVQ, POPQ, RET]) => i.dmem.read;

// Set write control signal
mem_write bool := mtc(M.icode, [RMMOVQ, PUSHQ, CALL]) => i.dmem.write;

mem_datain u64 := M.vala => i.dmem.datain;

// Update the status
m_stat Stat = [
    o.dmem.error => Stat::Adr;
    1 => M.stat;
] => i.w.stat;

m_icode u8 := M.icode => i.w.icode;

m_valm u64 := o.dmem.dataout => i.w.valm;
m_vale u64 := M.vale => i.w.vale;
m_dste u8 := M.dste => i.w.dste;
m_dstm u8 := M.dstm => i.w.dstm;

/////////////////// Write back stage ///////////////////

// Set E port register ID
w_dste u8 := W.dste;

// Set E port value
w_vale u64 := W.vale;

// Set M port register ID
w_dstm u8 := W.dstm;

// Set M port value
w_valm u64 := W.valm;

// Update processor status
overall_stat Stat = [
    W.stat == Stat::Bub => Stat::Aok;
    1 => W.stat;
];

///////////////// Pipeline Register Control /////////////////////////

// Should I stall or inject a bubble into Pipeline Register F?
// At most one of these can be true.
f_bubble bool := false => i.f.bubble;
f_stall bool :=
	// Conditions for a load/use hazard
	mtc(E.icode, [ MRMOVQ, POPQ ]) &&
	 mtc(E.dstm, [ c.d_srca, c.d_srcb ]) ||
	// Stalling at fetch while ret passes through pipeline
	mtc(RET, [D.icode, E.icode, M.icode])
    => i.f.stall;

// Should I stall or inject a bubble into Pipeline Register D?
// At most one of these can be true.
d_stall bool := 
	// Conditions for a load/use hazard
	mtc(E.icode, [MRMOVQ, POPQ]) &&
	mtc(E.dstm, [c.d_srca, c.d_srcb])
    => i.d.stall;

d_bubble bool :=
	// Mispredicted branch
	(E.icode == JX && !c.e_cnd) ||
	// Stalling at fetch while ret passes through pipeline
	// but not condition for a load/use hazard
	!(mtc(E.icode, [ MRMOVQ, POPQ]) && mtc(E.dstm, [c.d_srca, c.d_srcb])) &&
	  mtc(RET, [D.icode, E.icode, M.icode])
    => i.d.bubble;

// Should I stall or inject a bubble into Pipeline Register E?
// At most one of these can be true.
e_stall bool := false => i.e.stall;
e_bubble bool :=
	// Mispredicted branch
	(E.icode == JX && !c.e_cnd) ||
	// Conditions for a load/use hazard
	mtc(E.icode, [MRMOVQ, POPQ]) &&
	mtc(E.dstm, [c.d_srca, c.d_srcb])
    => i.e.bubble;

// Should I stall or inject a bubble into Pipeline Register M?
// At most one of these can be true.
m_stall bool := false => i.m.stall;
// Start injecting bubbles as soon as exception passes through memory stage
m_bubble bool := 
    mtc(c.m_stat, [Stat::Adr, Stat::Ins, Stat::Hlt]) 
    || mtc(W.stat, [Stat::Adr, Stat::Ins, Stat::Hlt])
    => i.m.bubble;

// Should I stall or inject a bubble into Pipeline Register W?
w_stall bool := mtc(W.stat, [Stat::Adr, Stat::Ins, Stat::Hlt]) => i.w.stall;
w_bubble bool := false => i.w.bubble;
}

#[cfg(test)]
mod tests {
    use crate::pipeline::hardware::{DeviceInputSignal, DeviceOutputSignal, Devices};

    use super::{update, IntermediateSignal};

    #[test]
    fn test_hcl() {
        let mut inter = IntermediateSignal::default();
        let mut devin = DeviceInputSignal::default();
        let devout = DeviceOutputSignal::default();
        let mut devices = Devices::default();
        let out = update(&mut inter, &mut devin, devout, &mut devices);
        eprintln!("{:?}", out);
    }
}
