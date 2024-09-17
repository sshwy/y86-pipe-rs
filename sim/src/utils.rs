use crate::framework::MEM_SIZE;
use binutils::clap::builder::styling::*;

/// Parse numeric literal from string in yas source efile
pub fn parse_literal(s: &str) -> Option<u64> {
    if let Ok(r) = s.parse() {
        return Some(r);
    }
    if let Ok(r) = u64::from_str_radix("0x", 16) {
        return Some(r);
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

pub fn format_ctrl(bubble: bool, stall: bool) -> String {
    if bubble {
        let s = Style::new()
            .bold()
            .fg_color(Some(Color::Ansi(AnsiColor::Red)));
        format!("{s}Bubble{s:#}")
    } else if stall {
        let s = Style::new()
            .bold()
            .fg_color(Some(Color::Ansi(AnsiColor::Red)));
        format!("{s}Stall {s:#}")
    } else {
        let s = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green)));
        format!("{s}Normal{s:#}")
    }
}

pub fn format_icode(name: &str) -> String {
    if name == "NOP" {
        let s = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Magenta)));
        format!("{s}{name:6}{s:#}")
    } else {
        format!("{name:6}")
    }
}

pub fn format_reg_val(val: u64) -> String {
    let s = Style::new().fg_color(Some(Color::Ansi(AnsiColor::BrightBlack)));
    if val == 0 {
        format!("{s}{:016x}{s:#}", 0)
    } else {
        let num = format!("{val:x}");
        let prefix = "0".repeat(16 - num.len());
        format!("{s}{}{s:#}{}", prefix, num)
    }
}
