# Y86 Pipeline Simulator Rust Implementation

Y86-64 is an ISA described in CMU CS15213: CSAPP 
(or Introduction to Computer System, ICS, 计算机系统导论 in chinese).

This project reimplement the y86 simulator in
the ArchLab using Rust, aiming at better code readability
and experiment for mordern solutions for DSL handling.

Without using flex, yacc or bison for HCL (hardware control
language) parsing, we tend to a pure rust macro rule approach,
which provides not only similar syntax with the original HCL, 
but also zero-cost dev-time syntax highlighting and type checking.

Moreover, we're able to build the computational graph 
(only once before the first cycle) as a DAG and arrange the order of
execution of combinatorial logics and units topologically,
without explicitly writing code for stepping cycles.

Hope you enjoy.

Note: the behavior of `cargo-expand` is incorrect.

to test: RUST_MIN_STACK=104857600 cargo test