use crate::hcl;

hcl! {
    @hardware crate::pipeline::hardware;
    @devinput i;     // input of a device, or next stage state (fdemw)
    @devoutput o;    // output of a device, or current stage state (FDEMW)
    @intermediate c; // intermediate value
    @abbr F D E M W

    @use crate::pipeline::Stat;

    // Fetch stage

    f_pc u64 = [
        // Mispredicted branch.  Fetch at incremented PC
        M.icode == JX && M.cnd == 0 => M.vala;
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
    instr_valid bool := mtc(c.f_icode, [NOP, HALT, CMOVX, IRMOVQ, RMMOVQ, MRMOVQ,
        OPQ, JX, CALL, RET, PUSHQ, POPQ]);

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
        dbg!(out);
    }
}
