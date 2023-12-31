use crate::isa::BIN_SIZE;

// little endian
pub fn get_u64(binary: &[u8]) -> u64 {
    let mut res = 0;
    for (i, byte) in binary.iter().enumerate().take(8) {
        res += (*byte as u64) << (i * 8);
    }
    res
}
pub fn put_u64(binary: &mut [u8], val: u64) {
    for (i, byte) in binary.iter_mut().enumerate().take(8) {
        *byte = (val >> (i * 8)) as u8;
    }
}

pub fn mem_diff(left: &[u8; BIN_SIZE], right: &[u8; BIN_SIZE]) {
    for i in 0..BIN_SIZE >> 3 {
        if get_u64(&left[i << 3..]) != get_u64(&right[i << 3..]) {
            eprint!("{:#06x}: ", i << 3,);
            for byte in left[i << 3..].iter().take(8) {
                eprint!("{:02x}", *byte)
            }
            eprint!(" -> ");
            for byte in right[i << 3..].iter().take(8) {
                eprint!("{:02x}", *byte)
            }
            eprintln!()
        }
    }
}

pub fn mem_print(bin: &[u8; BIN_SIZE]) {
    let mut max_i = 0;
    for i in 0..BIN_SIZE >> 3 {
        if get_u64(&bin[i << 3..]) != 0 {
            max_i = i;
        }
    }
    for i in 0..=max_i {
        eprint!("{:#06x}: ", i << 3);
        for byte in bin[i << 3..].iter().take(8) {
            eprint!("{:02x}", *byte)
        }
        eprintln!()
    }
}
