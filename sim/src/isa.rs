//! Instruction Set definition for Y86-64 Architecture

use crate::{
    object::BIN_SIZE,
    utils::{format_reg_file, get_u64, mem_diff, put_u64},
};

macro_rules! define_code {
    {
        @mod $modname:ident;
        @type $typ:ty;
        $( $cname:ident = $cval:expr; )*
    } => {
        pub mod $modname {
            $(pub const $cname : $typ = $cval; )*
            #[allow(unused)]
            pub fn name_of(code: $typ) -> &'static str {
                match code {
                    $($cname => stringify!($cname), )*
                    _ => "no name"
                }
            }
        }
    };
}

define_code! {
    @mod inst_code;
    @type u8;
    HALT = 0x0;
    NOP = 0x1;
    CMOVX = 0x2;
    IRMOVQ = 0x3;
    RMMOVQ = 0x4;
    MRMOVQ = 0x5;
    OPQ = 0x6;
    JX = 0x7;
    CALL = 0x8;
    RET = 0x9;
    PUSHQ = 0xa;
    POPQ = 0xb;
    // extended instruction
    IOPQ = 0xc;
}

define_code! {
    @mod reg_code;
    @type u8;
    RAX = 0;
    RCX = 1;
    RDX = 2;
    RBX = 3;
    RSP = 4;
    RBP = 5;
    RSI = 6;
    RDI = 7;
    R8 = 8;
    R9 = 9;
    R10 = 0xa;
    R11 = 0xb;
    R12 = 0xc;
    R13 = 0xd;
    R14 = 0xe;
    RNONE = 0xf;
}

/// we use a 64-bit integer array of length 16 to represent the register file.
pub type RegFile = [u64; 16];

define_code! {
    @mod op_code;
    @type u8;
    ADD = 0;
    SUB = 1;
    AND = 2;
    XOR = 3;
}

pub fn arithmetic_compute(a: u64, b: u64, op: u8) -> Option<u64> {
    use op_code::*;
    match op {
        ADD => Some(b.wrapping_add(a)),
        SUB => Some(b.wrapping_sub(a)),
        XOR => Some(b ^ a),
        AND => Some(b & a),
        _ => None,
    }
}

define_code! {
    @mod cond_fn;
    @type u8;
    YES = 0;
    LE = 1;
    L = 2;
    E = 3;
    NE = 4;
    GE = 5;
    G = 6;
}

/// A data structure that simulates the condition codes.
#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ConditionCode {
    pub sf: bool,
    pub of: bool,
    pub zf: bool,
}

/// Default value of the condition code.
pub const CC_INIT: ConditionCode = ConditionCode {
    sf: false,
    of: false,
    zf: false,
};

impl ConditionCode {
    /// Test if the condition code satisfies the given condition function.
    pub fn test(self, cfn: u8) -> bool {
        let Self { sf, zf, of } = self;
        use cond_fn::*;
        match cfn {
            YES => true,
            E => zf,
            NE => !zf,
            L => sf ^ of,
            LE => zf || (sf ^ of),
            GE => !(sf ^ of),
            G => !zf && !(sf ^ of),
            _ => false,
        }
    }

    pub fn set(&mut self, a: u64, b: u64, e: u64, opfun: u8) {
        const W_1: usize = std::mem::size_of::<u64>() * 8 - 1;
        use op_code::*;
        *self = ConditionCode {
            sf: (e >> W_1 & 1) != 0,
            zf: e == 0,
            of: match opfun {
                // a, b have the same sign and a, e have different sign
                ADD => (!(a ^ b) & (a ^ e)) >> W_1 != 0,
                // (b - a): a, b have different sign and b, e have different sign
                SUB => ((a ^ b) & (b ^ e)) >> W_1 != 0,
                _ => false,
            },
        };
    }
}

impl std::fmt::Display for ConditionCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s_true = format!("{s}true{s:#}", s = crate::utils::GRNB);
        let s_false = format!("{s}false{s:#}", s = crate::utils::GRAY);
        write!(
            f,
            "sf {sf}  of {of}  zf {zf}",
            sf = if self.sf { &s_true } else { &s_false },
            of = if self.of { &s_true } else { &s_false },
            zf = if self.zf { &s_true } else { &s_false },
        )
    }
}

/// Simulator State (at each stage), depending on the hardware design.
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum Stat {
    /// Indicates that everything is fine.
    Aok = 0,
    /// Indicates that the stage is bubbled. A bubbled stage execute the NOP
    /// instruction. Initially, all stages are in the bubble state.
    Bub = 1,
    /// The halt state. This state is assigned when the instruction fetcher
    /// reads the halt instruction. (If your architecture lacks a
    /// instruction fetcher, there should be some other way to specify the
    /// halt state in HCL.)
    Hlt = 2,
    /// This state is assigned when the instruction memory or data memory is
    /// accessed with an invalid address.
    Adr = 3,
    /// This state is assigned when the instruction fetcher reads an invalid
    /// instruction code.
    Ins = 4,
}

impl Default for Stat {
    fn default() -> Self {
        Self::Aok
    }
}

impl std::fmt::Display for Stat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (name, s) = match self {
            Stat::Aok => ("aok", crate::utils::GRN),
            Stat::Bub => ("bub", crate::utils::GRAY),
            Stat::Hlt => ("hlt", crate::utils::GRNB),
            Stat::Adr => ("adr", crate::utils::REDB),
            Stat::Ins => ("ins", crate::utils::REDB),
        };
        write!(f, "{s}{name}{s:#}")
    }
}

/// Simulation result of the Y86 machine code on the standard ISA.
pub struct StandardResult {
    pub bin: [u8; BIN_SIZE],
    pub cc: ConditionCode,
    pub regs: RegFile,
    pub pc: usize,
    pub n_insts: u64,
}

/// Execute Y86 machine code w.r.t. the ISA specification. This function
/// is used to verify the correctness of the pipeline architectures.
///
/// It supports the extended `iopq` instruction.
pub fn simulate(mut bin: [u8; BIN_SIZE], tty_out: bool) -> anyhow::Result<StandardResult> {
    let original = bin;
    let mut pc = 0;

    fn ensure_reg(reg: u8) -> anyhow::Result<usize> {
        if reg >= 16 {
            anyhow::bail!("invalid register code: {:#x}", reg);
        }
        Ok(reg as usize)
    }

    // Condition code register
    let mut reg_cc = ConditionCode::default();
    let mut reg_file = [0u64; 16];

    let mut n_insts = 0;

    loop {
        n_insts += 1;
        let icode = bin[pc] >> 4;
        let ifun = bin[pc] & 0xf;

        use crate::utils::GRAY;

        macro_rules! print_inst {
            () => {
                if tty_out {
                    print!(
                        "{GRAY}{:#06x}{GRAY:#}  icode: {icode:#x} ({}), ifun: {}",
                        pc,
                        inst_code::name_of(icode),
                        ifun
                    );
                }
            };
        }

        macro_rules! print_reg {
            () => {
                if tty_out {
                    print!(
                        ", rA: {}, rB: {}",
                        reg_code::name_of(bin[pc + 1] >> 4),
                        reg_code::name_of(bin[pc + 1] & 0xf)
                    );
                }
            };
        }

        macro_rules! print_v {
            ($v:expr) => {
                if tty_out {
                    print!(", V: {:#x}", $v);
                }
            };
        }

        macro_rules! print_end {
            () => {
                if tty_out {
                    println!();
                }
            };
        }

        let nex_pc = match icode {
            inst_code::HALT => {
                if ifun != 0 {
                    anyhow::bail!("invalid ifun for HALT: {:#x}", ifun);
                }
                print_inst!();
                print_end!();
                break;
            }
            inst_code::NOP => {
                let nex_pc = pc.checked_add(1).ok_or(anyhow::anyhow!("pc overflow"))?;
                if ifun != 0 {
                    anyhow::bail!("invalid ifun for NOP: {:#x}", ifun);
                }
                print_inst!();
                print_end!();
                nex_pc
            }
            inst_code::CMOVX => {
                let nex_pc = pc.checked_add(2).ok_or(anyhow::anyhow!("pc overflow"))?;
                let ra = ensure_reg(bin[pc + 1] >> 4)?;
                let rb = ensure_reg(bin[pc + 1] & 0xf)?;

                print_inst!();
                print_reg!();
                print_end!();

                if reg_cc.test(ifun) {
                    reg_file[rb] = reg_file[ra];
                }
                nex_pc
            }
            inst_code::IRMOVQ => {
                let nex_pc = pc.checked_add(10).ok_or(anyhow::anyhow!("pc overflow"))?;
                if ifun != 0 {
                    anyhow::bail!("invalid ifun for IRMOVQ: {:#x}", ifun);
                }

                let ra = bin[pc + 1] >> 4;
                if ra != reg_code::RNONE {
                    anyhow::bail!("invalid register code: {:#x}", ra);
                }
                let rb = ensure_reg(bin[pc + 1] & 0xf)?;
                let v = get_u64(&bin[(pc + 2)..(pc + 10)]);

                print_inst!();
                print_reg!();
                print_v!(v);
                print_end!();

                reg_file[rb] = v;

                nex_pc
            }
            inst_code::RMMOVQ => {
                let nex_pc = pc.checked_add(10).ok_or(anyhow::anyhow!("pc overflow"))?;
                if ifun != 0 {
                    anyhow::bail!("invalid ifun for RMMOVQ: {:#x}", ifun);
                }

                print_inst!();
                print_reg!();
                print_end!();

                let ra = ensure_reg(bin[pc + 1] >> 4)?;
                let rb = ensure_reg(bin[pc + 1] & 0xf)?;
                let v = get_u64(&bin[(pc + 2)..(pc + 10)]);

                let addr = (reg_file[rb] as i64 + v as i64) as usize;
                if addr >= BIN_SIZE {
                    anyhow::bail!("invalid memory address: {:#x}", addr);
                }
                put_u64(&mut bin[addr..(addr + 8)], reg_file[ra]);

                nex_pc
            }
            inst_code::MRMOVQ => {
                let nex_pc = pc.checked_add(10).ok_or(anyhow::anyhow!("pc overflow"))?;
                if ifun != 0 {
                    anyhow::bail!("invalid ifun for MRMOVQ: {:#x}", ifun);
                }
                print_inst!();
                print_reg!();
                print_end!();

                let ra = ensure_reg(bin[pc + 1] >> 4)?;
                let rb = ensure_reg(bin[pc + 1] & 0xf)?;
                let v = get_u64(&bin[(pc + 2)..(pc + 10)]);

                let addr = (reg_file[rb] as i64 + v as i64) as usize;
                if addr >= BIN_SIZE {
                    anyhow::bail!("invalid memory address: {:#x}", addr);
                }
                reg_file[ra] = get_u64(&bin[addr..(addr + 8)]);

                nex_pc
            }
            inst_code::OPQ => {
                let nex_pc = pc.checked_add(2).ok_or(anyhow::anyhow!("pc overflow"))?;
                let ra = ensure_reg(bin[pc + 1] >> 4)?;
                let rb = ensure_reg(bin[pc + 1] & 0xf)?;
                print_inst!();
                print_reg!();
                print_end!();

                let va = reg_file[ra];
                let vb = reg_file[rb];

                let Some(ve) = arithmetic_compute(va, vb, ifun) else {
                    anyhow::bail!("invalid ifun for OPQ: {:#x}", ifun);
                };
                reg_cc.set(va, vb, ve, ifun);
                reg_file[rb] = ve;

                nex_pc
            }
            inst_code::JX => {
                let nex_pc = pc.checked_add(9).ok_or(anyhow::anyhow!("pc overflow"))?;
                let v = get_u64(&bin[(pc + 1)..(pc + 9)]);

                print_inst!();
                print_v!(v);
                print_end!();

                if reg_cc.test(ifun) {
                    v as usize
                } else {
                    nex_pc
                }
            }
            inst_code::CALL => {
                if ifun != 0 {
                    anyhow::bail!("invalid ifun for CALL: {:#x}", ifun);
                }

                let v = get_u64(&bin[(pc + 1)..(pc + 9)]);

                print_inst!();
                print_v!(v);
                print_end!();

                let rsp = reg_file.get_mut(reg_code::RSP as usize).unwrap();
                *rsp = rsp.checked_sub(8).ok_or(anyhow::anyhow!("rsp overflow"))?;
                put_u64(
                    &mut bin[(*rsp as usize)..(*rsp as usize + 8)],
                    pc as u64 + 9,
                );

                v as usize
            }
            inst_code::RET => {
                if ifun != 0 {
                    anyhow::bail!("invalid ifun for RET: {:#x}", ifun);
                }
                print_inst!();
                print_end!();

                let rsp = reg_file.get_mut(reg_code::RSP as usize).unwrap();
                let v = get_u64(&bin[(*rsp as usize)..(*rsp as usize + 8)]);

                *rsp = rsp.checked_add(8).ok_or(anyhow::anyhow!("rsp overflow"))?;

                v as usize
            }
            inst_code::PUSHQ => {
                let nex_pc = pc.checked_add(2).ok_or(anyhow::anyhow!("pc overflow"))?;
                if ifun != 0 {
                    anyhow::bail!("invalid ifun for PUSHQ: {:#x}", ifun);
                }
                print_inst!();
                print_reg!();
                print_end!();

                let ra = ensure_reg(bin[pc + 1] >> 4)?;
                let va = reg_file[ra];

                let rsp = reg_file.get_mut(reg_code::RSP as usize).unwrap();
                *rsp = rsp.checked_sub(8).ok_or(anyhow::anyhow!("rsp overflow"))?;
                let new_rsp = *rsp as usize;
                put_u64(&mut bin[new_rsp..(new_rsp + 8)], va);

                nex_pc
            }
            inst_code::POPQ => {
                let nex_pc = pc.checked_add(2).ok_or(anyhow::anyhow!("pc overflow"))?;
                if ifun != 0 {
                    anyhow::bail!("invalid ifun for POPQ: {:#x}", ifun);
                }
                print_inst!();
                print_reg!();
                print_end!();

                let ra = ensure_reg(bin[pc + 1] >> 4)?;

                let rsp = reg_file.get_mut(reg_code::RSP as usize).unwrap();
                let old_rsp = *rsp as usize;
                *rsp = rsp.checked_add(8).ok_or(anyhow::anyhow!("rsp overflow"))?;
                reg_file[ra] = get_u64(&bin[old_rsp..(old_rsp + 8)]);

                nex_pc
            }
            // extended instruction
            // iopq v, rb
            inst_code::IOPQ => {
                let nex_pc = pc.checked_add(10).ok_or(anyhow::anyhow!("pc overflow"))?;
                let ra = bin[pc + 1] >> 4;
                if ra != reg_code::RNONE {
                    anyhow::bail!("invalid register code: {:#x}", ra);
                }
                let rb = ensure_reg(bin[pc + 1] & 0xf)?;
                let vb = reg_file[rb];
                let v = get_u64(&bin[(pc + 2)..(pc + 10)]);

                print_inst!();
                print_reg!();
                print_v!(v);
                print_end!();

                let Some(ve) = arithmetic_compute(v, vb, ifun) else {
                    anyhow::bail!("invalid ifun for IOPQ: {:#x}", ifun);
                };
                reg_cc.set(v, vb, ve, ifun);
                reg_file[rb] = ve;

                nex_pc
            }
            _ => anyhow::bail!("unknown icode: {:#x}", icode),
        };
        pc = nex_pc;
    }

    if tty_out {
        println!();
        println!("total instructions: {}", n_insts);
        println!("{}", format_reg_file(reg_file));
        mem_diff(&original, &bin);
    }

    Ok(StandardResult {
        bin,
        cc: reg_cc,
        regs: reg_file,
        pc,
        n_insts,
    })
}
