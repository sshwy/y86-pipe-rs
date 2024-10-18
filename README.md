# y86-pipe-rs: Y86-64 Processor Simulator written in Rust

This document describes the processor simulators that accompany the presentation of the Y86-64 processor architectures in Chapter 4 of _Computer Systems: A Programmer’s Perspective, Third Edition_.

The original (official) simulator, written in C has difficulty adapting to too many modifications on the seq, seq+ and pipe HCL, leading to a limited range of lab assignments. This project aims to provide a more flexible and extensible simulator for the Y86-64 processor, and is employed in Peking U's _ICS: Introduction to Computer System_ in 2024.

## Installation

This project is written in Rust, so you'd have your Rust toolchain installed. If you haven't, please execute the following command to install [rustup](https://rustup.rs/):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

You can verify the installation by executing the command `rustup`.

Install and the Rust toolchain by executing the following command (by the time of writing, the latest stable version is 1.81):

```bash
rustup install 1.81
rustup default 1.81
```

## Build the Project

Simply execute `cargo build` to build all the binaries in the project. After running this command, a folder named `target` will be created to store the output binaries and other intermediate files. The output executables are

- `target/debug/yas`: Y86-64 Assembler
- `target/debug/yis`: Y86-64 ISA Simulator
- `target/debug/ysim`: Y86-64 Pipline Simulator
- `target/debug/ydb`: Y86-64 Debugger Server

To build the release version, execute `cargo build --release`. The release version is optimized for performance. The released version executables locate in the `target/release` folder (`target/release/{yas,yis,ysim,ydb}`).

## Assembler Usage

To assemble a Y86-64 assembly file, execute the following command:

```bash
./target/debug/yas [input_file].ys
```

The default output filename is `[input_file].yo`. You can specify the output filename by adding the `-o` option. For example, given the following y86 assembly file `swap.ys`:

```asm
# Swap nums if the former one >= the latter one
    .pos 0
    irmovq stack, %rsp

    irmovq nums, %rdi
    mrmovq (%rdi), %rdx
    mrmovq 8(%rdi), %rcx
    rrmovq %rdx, %rbp
    subq %rcx, %rbp # $rbp <= $rcx ?
    # if so, then do not swap
    jle done
    rmmovq %rdx, 8(%rdi)
    rmmovq %rcx, (%rdi)
done:
    halt
    nop
    nop
    nop

    .align 8
nums:
    .quad 0xcba
    .quad 0xbca
    
    .pos 0x200
stack:
```

By running `./target/debug/yas swap.ys`, the assembler will generate a binary file `swap.yo`:

```asm
                             | # Swap nums if the former one >= the latter one
0x0000:                      |     .pos 0
0x0000: 30f40002000000000000 |     irmovq stack, %rsp
                             | 
0x000a: 30f75000000000000000 |     irmovq nums, %rdi
0x0014: 50270000000000000000 |     mrmovq (%rdi), %rdx
0x001e: 50170800000000000000 |     mrmovq 8(%rdi), %rcx
0x0028: 2025                 |     rrmovq %rdx, %rbp
0x002a: 6115                 |     subq %rcx, %rbp # $rbp <= $rcx ?
                             |     # if so, then do not swap
0x002c: 714900000000000000   |     jle done
0x0035: 40270800000000000000 |     rmmovq %rdx, 8(%rdi)
0x003f: 40170000000000000000 |     rmmovq %rcx, (%rdi)
0x0049:                      | done:
0x0049: 00                   |     halt
0x004a: 10                   |     nop
0x004b: 10                   |     nop
0x004c: 10                   |     nop
                             | 
0x0050:                      |     .align 8
0x0050:                      | nums:
0x0050: ba0c000000000000     |     .quad 0xcba
0x0058: ca0b000000000000     |     .quad 0xbca
                             |     
0x0200:                      |     .pos 0x200
0x0200:                      | stack:
                             | 
```
## Simulator Usage

To simulate a Y86-64 assembly file, execute the following command:

```bash
./target/debug/ysim [input_file].ys
```

This will print the state of the processor at each cycle to the standard output. If you want to read tht output from start to end, you can pipe a `less` command to the output (To quit `less`, press `q`):

```bash
./target/debug/ysim [input_file].ys | less
```

To print more information you can use the `-v` option, which will display the value of each variable in each stage of the cycle:

```bash
./target/debug/ysim [input_file].ys -v
```

We provide different architectures for the simulator. To view available architectures, you can run

```bash
./target/debug/ysim --help
```

To specify an architecture, you can use the `--arch` option. For example, to run the simulator with the `seq_plus_std` architecture, you can run:

```bash
./target/debug/ysim [input_file].ys --arch seq_plus_std
```

## Code Organization and Custom Architectures

By default we simulate the `seq` architecture. For a processor architecture, we define its hardware components using the `define_units` macro in the `sim/src/architectures/hardware_seq.rs` module. Its functionality is implemented in Rust. 

We use Rust macros to parse the HCL that defines the architecture of the Y86-64 processor. Therefore you can define your own architecture without writing even a single line of Rust code!

However, the original HCL does not declare the relation of the variables and CPU hardware devices inputs clearly. To address this ambiguity, we modify the HCL syntax to unvail these information.

Besides, it is important to remember some basic types in Rust to define variables in the HCL macro block. The Rust basic types used in HCL and their corresponding equivalent C/C++ types are:

| Rust Type | C/C++ Type           |
|-----------|----------------------|
| `u8`      | `unsigned char`      |
| `u64`     | `unsigned long long` |
| `bool`    | `bool` (in C++)      |

There are some other data structures. You can inspect their definition in `sim/src/isa.rs` (The syntax of Rust is similar to C/C++, so you can easily understand the code):


| Rust Type       |  Description                     |
|-----------------|----------------------------------|
| `ConditionCode` | Flags stored in the cc register. |
| `Stat`          | Status of the stage.             |

We also exported a useful constant `NEG_8` to represent `-8` in 64-bit (`0xFFFFFFFFFFFFFFF8`) to improve the readability of the HCL code.

To define your custom architecture, create a new `.rs` file in the `sim/src/architectures/extra` folder. The name of the file (do not include blank characters in the file name) will become the name of your architecture.

The hardware module you choose exports those units defined in the `define_units` macro as:


```
define_units! {
    UnitName unit_var_name {
        .input(
            input_field_name: filed_type,
            ...
        )
        .output(
            output_field_name: filed_type,
            ...
        )
    }
}
```

Then in your HCL macro block, you can read the output of a unit via:

```
var_type var_name = unit_var_name.output_field_name;

// or

var_type var_name = [
    some condition: unit_var_name.output_field_name;
];
```

If you still don't know how to define your architecture, read the source of builtin architectures for reference.

After defining your architecture, you need to rebuild the project. The new architecture will be available via the `--arch` option.