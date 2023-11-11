//! This module provides parsing utilities for the y86 assembly.
use pest_derive::Parser;



#[derive(Parser)]
#[grammar = "src/parse/grammer.pest"] // relative to src
pub struct Y86AsmParser;

