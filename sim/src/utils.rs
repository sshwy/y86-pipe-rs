use crate::framework::MEM_SIZE;

/// Parse numeric literal from string in yas source efile
pub fn parse_literal(s: &str) -> Option<u64> {
    if let Some(r) = s.parse().ok() {
        return Some(r);
    }
    if s.starts_with("0x") {
        return u64::from_str_radix(&s[2..], 16).ok();
    }
    None
}

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

pub fn mem_diff(left: &[u8; MEM_SIZE], right: &[u8; MEM_SIZE]) {
    for i in 0..MEM_SIZE >> 3 {
        if get_u64(&left[i << 3..]) != get_u64(&right[i << 3..]) {
            print!("{:#06x}: ", i << 3,);
            for byte in left[i << 3..].iter().take(8) {
                print!("{:02x}", *byte)
            }
            print!(" -> ");
            for byte in right[i << 3..].iter().take(8) {
                print!("{:02x}", *byte)
            }
            println!()
        }
    }
}

pub fn mem_print(bin: &[u8; MEM_SIZE]) {
    let mut max_i = 0;
    for i in 0..MEM_SIZE >> 3 {
        if get_u64(&bin[i << 3..]) != 0 {
            max_i = i;
        }
    }
    for i in 0..=max_i {
        print!("{:#06x}: ", i << 3);
        for byte in bin[i << 3..].iter().take(8) {
            print!("{:02x}", *byte)
        }
        println!()
    }
}
