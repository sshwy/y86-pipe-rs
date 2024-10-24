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

## ISA Simulator Usage

To simulate a Y86-64 assembly file w.r.t. the Y86 ISA specification, you can execute the following command:

```bash
./target/debug/yis [input_file].yo
```

For example, by running `./target/debug/yis swap.yo`, the simulator will print the following information:

```
0x0000  icode: 0x3 (IRMOVQ), ifun: 0, rA: RNONE, rB: RSP, V: 0x200
0x000a  icode: 0x3 (IRMOVQ), ifun: 0, rA: RNONE, rB: RDI, V: 0x50
0x0014  icode: 0x5 (MRMOVQ), ifun: 0, rA: RDX, rB: RDI
0x001e  icode: 0x5 (MRMOVQ), ifun: 0, rA: RCX, rB: RDI
0x0028  icode: 0x2 (CMOVX), ifun: 0, rA: RDX, rB: RBP
0x002a  icode: 0x6 (OPQ), ifun: 1, rA: RCX, rB: RBP
0x002c  icode: 0x7 (JX), ifun: 1, V: 0x49
0x0035  icode: 0x4 (RMMOVQ), ifun: 0, rA: RDX, rB: RDI
0x003f  icode: 0x4 (RMMOVQ), ifun: 0, rA: RCX, rB: RDI
0x0049  icode: 0x0 (HALT), ifun: 0

total instructions: 10
ax 0000000000000000 bx 0000000000000000 cx 0000000000000bca dx 0000000000000cba
si 0000000000000000 di 0000000000000050 sp 0000000000000200 bp 00000000000000f0
0x0050: ba0c000000000000 -> ca0b000000000000
0x0058: ca0b000000000000 -> ba0c000000000000
```

## Pipeline Simulator Usage

To simulate a Y86-64 assembly file over the default architecture (`seq_std`), execute the following command:

```bash
./target/debug/ysim [input_file].ys
```

This will print the state of the processor at each cycle to the standard output. If you want to read tht output from start to end, you can pipe a `less` command to the output (To quit `less`, press `q`):

```bash
./target/debug/ysim [input_file].ys | less
```

To print more information you can use the `-v` option, which will display the value of each signal in each stage of the cycle:

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

You can also inspect an architecture via `-I` option:

```bash
./target/debug/ysim --arch seq_plus_std -I
```

Its output will be like:

```
propagate order:
lv.1: pc
lv.2: imem align icode ifun instr_valid mem_read mem_write need_regids need_valC set_cc alufun
lv.3: ialign pc_inc dstM srcA srcB valC valP
lv.4: reg_read aluA mem_data aluB
lv.5: alu valE mem_addr
lv.6: reg_cc dmem cc valM stat prog_term
lv.7: cond cnd dstE
lv.8: reg_write
dependency graph visualization is generated at: seq_plus_std_dependency_graph.html
```

Here an HTML file is generated to visualize the dependency graph of the architecture. You can open the HTML file in a browser to view the dependency graph. In the graph, blue blocks are hardware components, and red blocks are signals. You can drag each block to change its position.

![](assets/visualization-screenshot.png)

## Debugger Usage

To provide a friendly coding experience, we develop a debugger server for the Y86 assembly language. This debugger server is used along with the `y86-debugger` VSCode extension.

First you should install `y86-debugger` extension from `assets/y86-debugger-0.2.0.vsix` (choose "Install from VSIX..."). To install the extension on SSH remote servers, you can first install it on your local machine, then open a remote host VSCode window and choose "Install in SSH:..." to install the extension on the remote server.

Then you should start the debugger server:

```bash
./target/debug/ydb -p 2345 # -p specifies the port number
```

After that you can start debugging in VSCode. You can set breakpoints, step through the code, and inspect the registers and memory. Click the debug icon at the right side of the menu bar to start debugging.

By default, your assembly file is simulated with the `seq_std` architecture. If you want to change the architecture, you may use the `--arch` option:

```bash
# specify another architecture to debug
# both builtin and extra architectures are supported
./target/debug/ydb -p 2345 --arch seq_plus_std
```

Refer to [y86-debugger](https://github.com/sshwy/y86-debugger) for more information.

![](assets/debugger-screenshot.png)

## HCL-rs Specification

Please refer to this [attachment](assets/hcl-rs.pdf) for detailed description of the HCL-rs syntax.