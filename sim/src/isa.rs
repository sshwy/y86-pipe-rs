//! Instruction Set definition for Y86-64 Architecture

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

define_code! {
    @mod op_code;
    @type u8;
    ADD = 0;
    SUB = 1;
    AND = 2;
    XOR = 3;
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
