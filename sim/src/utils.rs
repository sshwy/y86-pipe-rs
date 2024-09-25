use binutils::clap::builder::styling::*;

use crate::framework::MEM_SIZE;

pub const GRAY: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::BrightBlack)));
pub const RED: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Red)));
pub const REDB: Style = RED.bold();
pub const GRN: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green)));
pub const GRNB: Style = GRN.bold();
pub const B: Style = Style::new().bold();

/// Parse numeric literal from string in yas source file.
/// 
/// For decimal number, it should be a valid i64.
/// For hexadecimal number, it should be prefixed with "0x" and in range of u64.
pub fn parse_literal(s: &str) -> Option<u64> {
    let (sign, s) = s.strip_suffix("-").map(|s| (-1, s)).unwrap_or((1, s));
    if let Ok(r) = s.parse::<i64>() {
        return Some((r * sign) as u64);
    }
    if let Ok(r) = u64::from_str_radix(s.strip_prefix("0x")?, 16) {
        return Some((r as i64 * sign) as u64);
    }
    None
}

/// Get 64-bit unsigned integer value in little endian order.
pub fn get_u64(binary: &[u8]) -> u64 {
    let mut res = 0;
    for (i, byte) in binary.iter().enumerate().take(8) {
        res += (*byte as u64) << (i * 8);
    }
    res
}
/// Write 64-bit unsigned integer value to binary in little endian order.
pub fn put_u64(binary: &mut [u8], val: u64) {
    for (i, byte) in binary.iter_mut().enumerate().take(8) {
        *byte = (val >> (i * 8)) as u8;
    }
}

pub fn mem_diff(left: &[u8; MEM_SIZE], right: &[u8; MEM_SIZE]) {
    for i in 0..MEM_SIZE >> 3 {
        let offset = i << 3;
        if get_u64(&left[offset..]) != get_u64(&right[offset..]) {
            let l = &left[offset..offset + 8];
            let r = &right[offset..offset + 8];

            print!("{:#06x}: ", offset);
            for i in 0..8 {
                let s = if l[i] != r[i] { REDB } else { GRAY };
                print!("{s}{:02x}{s:#}", l[i])
            }
            print!(" -> ");
            for i in 0..8 {
                let s = if l[i] != r[i] { GRNB } else { GRAY };
                print!("{s}{:02x}{s:#}", r[i])
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

pub fn format_ctrl(bubble: bool, stall: bool) -> String {
    if bubble {
        format!("{REDB}Bubble{REDB:#}")
    } else if stall {
        format!("{REDB}Stall {REDB:#}")
    } else {
        format!("{GRN}Normal{GRN:#}")
    }
}

pub fn format_icode(icode: u8) -> String {
    let name = crate::isa::inst_code::name_of(icode);
    if name == "NOP" {
        format!("{GRAY}{name:6}{GRAY:#}")
    } else {
        format!("{name:6}")
    }
}

pub fn format_reg_val(val: u64) -> String {
    if val == 0 {
        format!("{GRAY}{:016x}{GRAY:#}", 0)
    } else {
        let num = format!("{val:x}");
        let prefix = "0".repeat(16 - num.len());
        format!("{GRAY}{}{GRAY:#}{B}{}{B:#}", prefix, num)
    }
}
