use binutils::clap::builder::styling::*;

use crate::{framework::MEM_SIZE, isa::RegFile};

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

/// Decode hexadecimal string to byte array.
pub fn decode_hex(s: &str) -> Result<Vec<u8>, std::num::ParseIntError> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
        .collect()
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

pub fn format_reg_file(reg_file: RegFile) -> String {
    fn format_reg_val(val: u64) -> String {
        if val == 0 {
            format!("{GRAY}{:016x}{GRAY:#}", 0)
        } else {
            let num = format!("{val:x}");
            let prefix = "0".repeat(16 - num.len());
            format!("{GRAY}{}{GRAY:#}{B}{}{B:#}", prefix, num)
        }
    }

    use crate::isa::reg_code::*;
    format!(
        "ax {rax} bx {rbx} cx {rcx} dx {rdx}\nsi {rsi} di {rdi} sp {rsp} bp {rbp}",
        rax = format_reg_val(reg_file[RAX as usize]),
        rbx = format_reg_val(reg_file[RBX as usize]),
        rcx = format_reg_val(reg_file[RCX as usize]),
        rdx = format_reg_val(reg_file[RDX as usize]),
        rsi = format_reg_val(reg_file[RSI as usize]),
        rdi = format_reg_val(reg_file[RDI as usize]),
        rsp = format_reg_val(reg_file[RSP as usize]),
        rbp = format_reg_val(reg_file[RBP as usize]),
    )
}

pub fn render_arch_dependency_graph(
    arch_name: &str,
    order: &crate::framework::PropOrder,
) -> anyhow::Result<()> {
    use charming::{
        element::{Color, Emphasis, EmphasisFocus},
        series::{Sankey, SankeyNode},
        Chart, HtmlRenderer,
    };

    fn map_name(name: &str) -> String {
        name.replace(".", "-")
    }

    let mut nodes = order
        .order
        .iter()
        .map(|o| o.name)
        .chain(order.edges.iter().map(|e| e.0.as_str()))
        .chain(order.edges.iter().map(|e| e.1.as_str()))
        .map(map_name)
        .collect::<Vec<_>>();

    // dedup the nodes
    nodes.sort();
    nodes.dedup();

    let devices_nodes = order
        .order
        .iter()
        .filter_map(|o| o.is_unit.then_some(o.name))
        .collect::<Vec<_>>();

    let nodes = nodes
        .into_iter()
        .map(|name| {
            let is_device = devices_nodes.contains(&name.as_str());
            let color = if is_device {
                Color::Value("rgba(0,0,255,0.5)".to_string())
            } else {
                Color::Value("rgba(255,0,0,0.5)".to_string())
            };
            SankeyNode::new(name).item_style(color)
        })
        .collect();

    let links = order
        .edges
        .iter()
        .map(|(from, to)| (map_name(from), map_name(to), 1))
        .collect();

    let c = Chart::new().series(
        Sankey::new()
            .emphasis(Emphasis::new().focus(EmphasisFocus::Adjacency))
            .data(nodes)
            .links(links),
    );

    let mut r = HtmlRenderer::new("Architecture Computational Dependency Graph", 1200, 800);
    let outpath = format!("{}_dependency_graph.html", arch_name);
    println!(
        "dependency graph visualization is generated at: {}",
        outpath
    );
    r.save(&c, outpath)?;

    Ok(())
}
