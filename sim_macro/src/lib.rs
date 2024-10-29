use std::{env, path::PathBuf};

use expr::LValue;
use items::{SignalDef, SignalSourceExpr, SignalSwitch};
use quote::{format_ident, quote, ToTokens};
use syn::{parse::Parse, parse_quote, punctuated::Punctuated, Token};
mod expr;
mod items;

struct HclData {
    hardware: syn::ExprPath,
    program_counter: LValue,
    termination: LValue,
    /// (cur, pre)
    stage_alias: items::StageAlias,
    stage_decls: Vec<items::StageDecl>,
    use_items: Vec<syn::ItemUse>,
    intermediate_signals: Vec<items::SignalDef>,
}

impl Parse for HclData {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        // Parse inner attributes
        let attrs = syn::Attribute::parse_inner(input)?;
        // find the hardware attribute
        let hardware = attrs
            .iter()
            .find_map(|attr| {
                if attr.path().is_ident("hardware") {
                    let value = &attr.meta.require_name_value().unwrap().value;
                    // parse value as path
                    let syn::Expr::Path(path) = value else {
                        panic!("hardware attribute must be a path");
                    };

                    Some(path)
                } else {
                    None
                }
            })
            .cloned()
            .unwrap();

        let stage_alias = attrs
            .iter()
            .find_map(|attr| {
                if attr.path().is_ident("stage_alias") {
                    let stage_alias = attr.parse_args::<items::StageAlias>().unwrap();

                    Some(stage_alias)
                } else {
                    None
                }
            })
            .unwrap_or_default();

        let program_counter = attrs
            .iter()
            .find_map(|attr| {
                if attr.path().is_ident("program_counter") {
                    let value = &attr.meta.require_name_value().unwrap().value;
                    // parse value as path
                    let syn::Expr::Path(path) = value else {
                        panic!("program_counter attribute must be a path");
                    };

                    Some(parse_quote!(#path))
                } else {
                    None
                }
            })
            .unwrap();

        let termination = attrs
            .iter()
            .find_map(|attr| {
                if attr.path().is_ident("termination") {
                    let value = &attr.meta.require_name_value().unwrap().value;
                    // parse value as path
                    let syn::Expr::Path(path) = value else {
                        panic!("program_counter attribute must be a path");
                    };

                    Some(parse_quote!(#path))
                } else {
                    None
                }
            })
            .unwrap();

        let mut use_items = Vec::new();
        let mut intermediate_signals = Vec::new();
        let mut stage_decls = Vec::new();
        let mut set_inputs = Vec::new();
        let mut set_stages = Vec::new();

        // repeatly parse the rest of the input
        loop {
            let lookahead = input.lookahead1();
            if input.is_empty() {
                break;
            } else if lookahead.peek(Token![use]) {
                let item = input.parse::<syn::ItemUse>()?;
                use_items.push(item);
            } else if lookahead.peek(Token![:]) {
                let item = input.parse::<items::StageDecl>()?;
                stage_decls.push(item);
            } else if lookahead.peek(Token![@]) {
                let _ = input.parse::<Token![@]>()?;
                let fn_name = input.parse::<syn::Ident>()?;
                if fn_name == "set_input" {
                    let unit_input = input.parse::<items::ComponentInputs>()?;
                    set_inputs.push(unit_input);
                    let _ = input.parse::<Token![;]>()?;
                } else if fn_name == "set_stage" {
                    let unit_input = input.parse::<items::ComponentInputs>()?;
                    set_stages.push(unit_input);
                    let _ = input.parse::<Token![;]>()?;
                } else {
                    panic!("unknown directive: {}", fn_name);
                }
            } else {
                let mut item = input.parse::<items::SignalDef>()?;
                if !stage_decls.is_empty() {
                    item.stage_index = Some(stage_decls.len() - 1);
                }
                item.destinations.iter_mut().for_each(|dest| {
                    if stage_alias.0.iter().any(|(cur, _)| cur == &dest.dest.0[0]) {
                        dest.is_stage_field = true;
                    }
                });
                intermediate_signals.push(item);
            }
        }

        for unit_input in set_inputs {
            let uname = unit_input.name;
            for fieldvalue in unit_input.fields {
                let sig = intermediate_signals
                    .iter_mut()
                    .find(|s| s.name == fieldvalue.1)
                    .unwrap();
                sig.destinations.push(items::SignalDest {
                    dest: LValue([uname.clone(), fieldvalue.0].into_iter().collect()),
                    tunnel: None,
                    is_stage_field: false,
                });
            }
        }
        for stage_input in set_stages {
            let uname = stage_input.name;
            for fieldvalue in stage_input.fields {
                let sig = intermediate_signals
                    .iter_mut()
                    .find(|s| s.name == fieldvalue.1)
                    .unwrap();
                sig.destinations.push(items::SignalDest {
                    dest: LValue([uname.clone(), fieldvalue.0].into_iter().collect()),
                    tunnel: None,
                    is_stage_field: true,
                });
            }
        }

        Ok(Self {
            stage_alias,
            hardware,
            program_counter,
            termination,
            use_items,
            intermediate_signals,
            stage_decls,
        })
    }
}

impl HclData {
    fn render_intermediate_signal_struct(&self) -> proc_macro2::TokenStream {
        let signal_fields: Punctuated<syn::Field, Token![,]> = self
            .intermediate_signals
            .iter()
            .map(|signal| -> syn::Field {
                let name = &signal.name;
                let typ = &signal.typ;
                parse_quote! { pub #name: #typ }
            })
            .collect();

        quote! {
            #[derive(Debug, Default, Clone)]
            #[allow(unused, non_snake_case)]
            #[cfg_attr(feature = "serde", derive(serde::Serialize))]
            pub struct IntermediateSignal {
                #signal_fields
            }
        }
    }

    fn render_signal_updater(
        signal: &SignalDef,
        expr_mapper: impl Fn(LValue) -> LValue + Clone,
        lval_mapper: impl Fn(LValue) -> LValue + Clone,
    ) -> proc_macro2::TokenStream {
        let name = &signal.name;

        let source_stmts = match &signal.source {
            items::SignalSource::Switch(SignalSwitch(cases)) => {
                let case_stmts = cases
                    .iter()
                    .map(|case| {
                        let cond = case.condition.clone().map(expr_mapper.clone());
                        let val = case.value.clone().map(expr_mapper.clone());
                        let tunnel_stmts = case.tunnel.as_ref().cloned().map(|tunnel| {
                            quote! {
                                has_tunnel_input = true;
                                tracing::debug!("tunnel triggered: {}", stringify!(#tunnel));
                                tracer.trigger_tunnel(stringify!(#tunnel));
                            }
                        });

                        quote! {
                            if (u8::from(#cond)) != 0 {
                                c_.#name = #val;
                                #tunnel_stmts
                            }
                        }
                    })
                    .reduce(|a, b| quote! { #a else #b })
                    .unwrap_or_default();

                quote! {
                    #case_stmts
                }
            }
            items::SignalSource::Expr(SignalSourceExpr { tunnel, expr }) => {
                let expr = expr.clone().map(expr_mapper.clone());
                let tunnel_stmts = tunnel.as_ref().cloned().map(|tunnel| {
                    quote! {
                        has_tunnel_input = true;
                        tracing::debug!("tunnel triggered: {}", stringify!(#tunnel));
                        tracer.trigger_tunnel(stringify!(#tunnel));
                    }
                });

                quote! {
                    c_.#name = #expr;
                    #tunnel_stmts
                }
            }
        };

        let dest_tunnel_stmts = signal
            .destinations
            .iter()
            .map(|dest| {
                let dst = dest.dest.clone().map(lval_mapper.clone());

                let tunner_stmt = dest.tunnel.as_ref().map(|tunnel| {
                    quote! {
                        if has_tunnel_input {
                            tracing::debug!("tunnel triggered: {}", stringify!(#tunnel));
                            tracer.trigger_tunnel(stringify!(#tunnel));
                        }
                    }
                });

                let dst_name = if dest.is_stage_field {
                    quote! { #dst }
                } else {
                    quote! { i_.#dst }
                };
                quote! {
                    #dst_name = c_.#name.to_owned();
                    #tunner_stmt
                }
            })
            .reduce(|a, b| quote! { #a #b })
            .unwrap_or_default();

        quote! {
            {
                fn updater(
                    i_: &mut UnitInputSignal,
                    c_: &mut IntermediateSignal,
                    n_: &mut PipeRegs,
                    tracer: &mut Tracer,
                    o_: &UnitOutputSignal,
                    p_: &PipeRegs,
                ) {
                    let mut has_tunnel_input = false;
                    #source_stmts
                    #dest_tunnel_stmts
                };
                circuit.add_update(stringify!(#name), updater);
            }
        }
    }

    fn render_build_circuit(&self) -> proc_macro2::TokenStream {
        let inter = &quote::format_ident!("c_");
        let inter_names = self
            .intermediate_signals
            .iter()
            .map(|s| &s.name)
            .collect::<Vec<_>>();
        let stage_alias = &self.stage_alias.0;

        let expr_mapper = |mut lv: LValue| -> LValue {
            if inter_names.contains(&&lv.0[0]) {
                lv.0.insert(0, inter.clone());
            } else if let Some((cur, _)) = stage_alias.iter().find(|(_, pre)| &lv.0[0] == pre) {
                lv.0[0] = cur.clone();
                lv.0.insert(0, format_ident!("p_"));
            } else if lv.0.len() > 1 {
                lv.0.insert(0, format_ident!("o_"));
            }
            lv
        };

        let lval_mapper = |mut lv: LValue| -> LValue {
            if inter_names.contains(&&lv.0[0]) {
                lv.0.insert(0, inter.clone());
            } else if let Some((cur, _)) = stage_alias.iter().find(|(cur, _)| &lv.0[0] == cur) {
                lv.0[0] = cur.clone();
                lv.0.insert(0, format_ident!("n_"));
            }
            lv
        };

        let updaters_stmt = self
            .intermediate_signals
            .iter()
            .map(|s| HclData::render_signal_updater(s, expr_mapper, lval_mapper))
            .reduce(|a, b| quote! { #a #b })
            .unwrap_or_default();

        let stmts =
            self.intermediate_signals
                .iter()
                .map(|signal| {
                    let name = &signal.name;
                    let update_stmts = signal
                    .source
                    .lvalues()
                    .into_iter()
                    .filter(|lv| {
                        // is intermediate signal or unit port
                        // previous stage fields are not included
                        lv.0.len() == 1 && inter_names.iter().any(|n| &lv.0[0] == *n)
                            || lv.0.len() == 2 && stage_alias.iter().all(|(_, pre)| &lv.0[0] != pre)
                    })
                    .map(|lv| quote! {
                        g.add_edge(stringify!(#lv).to_string(), stringify!(#name).to_string());
                    })
                    .reduce(|a, b| quote! { #a #b }).unwrap_or_default();
                    // .collect::<Punctuated<LValue, Token![,]>>();

                    let update_stmts = quote! {
                        #update_stmts
                        g.add_intermediate(stringify!(#name));
                    };
                    let rev_deps_stmts = signal
                        .destinations
                        .iter()
                        .map(move |dest| {
                            // the dest is either a stage name or input of a device
                            let dest_name = &dest.dest;

                            if dest.is_stage_field {
                                // dest is a stage output
                                // no edge is needed
                                quote! {}
                            } else {
                                // dest is a device input
                                quote! {
                                    g.add_edge(
                                        stringify!(#name).to_string(),
                                        stringify!(#dest_name).to_string()
                                    );
                                }
                            }
                        })
                        .reduce(|a, b| quote! { #a #b })
                        .unwrap_or_default();

                    quote! {
                        #update_stmts
                        #rev_deps_stmts
                    }
                })
                .reduce(|a, b| quote! { #a #b })
                .unwrap_or_default();

        quote! {
            fn build_circuit() -> crate::framework::PropCircuit<Arch> {
                use crate::framework::*;

                // cur: o, nex: i
                let order = {
                    let mut g = PropOrderBuilder::new();
                    // hardware setup
                    hardware_setup(&mut g);
                    #stmts
                    g.build()
                };

                use crate::isa::inst_code::*;
                use crate::isa::reg_code::*;
                use crate::isa::op_code::*;
                use binutils::clap::builder::styling::*;

                let mut circuit = PropCircuit::new(order);
                #updaters_stmt
                circuit
            }
        }
    }

    fn render_update(&self) -> proc_macro2::TokenStream {
        quote! {
            /// Simulate one cycle of the CPU, update the input and output signals
            /// of each unit. Return the stage registers for the next cycle, along
            /// with a tracer.
            #[allow(unused)]
            #[allow(non_snake_case)]
            fn update(&mut self) -> crate::framework::Tracer {
                let mut rcd = self.circuit.updates.make_propagator(
                    &mut self.cur_unit_in,
                    self.cur_unit_out.clone(),
                    &mut self.nex_state,
                    &self.cur_state,
                    &mut self.cur_inter
                );
                let units = &mut self.units;
                for item in &self.circuit.order.order {
                    if item.is_unit {
                        rcd.run_unit(|unit_in, unit_out| {
                            units.run(item.name, (unit_in, unit_out));
                        });
                    } else { // combinatorial logics do not change output (cur)
                        rcd.run_combinatorial_logic(item.name);
                    }
                }
                let (out, tracer) = rcd.finalize();
                self.cur_unit_out = out;
                tracer
            }
        }
    }

    fn render_get_stage_info(&self) -> proc_macro2::TokenStream {
        let mut stage_items = self
            .stage_decls
            .iter()
            .map(|stage| {
                let name = &stage.name;
                quote! {
                    crate::framework::StageInfo {
                        name: #name,
                        signals: vec![],
                    }
                }
            })
            .reduce(|a, b| quote! { #a, #b })
            .unwrap_or_default();
        if stage_items.is_empty() {
            stage_items = quote! {
                crate::framework::StageInfo {
                    name: "(default)",
                    signals: vec![],
                }
            };
        }
        let sig_stmts = self
            .intermediate_signals
            .iter()
            .map(|sig| {
                let stage_index = sig.stage_index.unwrap_or(0);
                let name = &sig.name;

                let formatted_val = if let syn::Type::Array(_) = &sig.typ {
                    quote! {
                        format!("{:x?}", self.cur_inter.#name)
                    }
                } else if let syn::Type::Path(p) = &sig.typ {
                    if ["u8", "u64"].into_iter().any(|t| p.path.is_ident(t)) {
                        quote! {
                            format!("{:#x?}", self.cur_inter.#name)
                        }
                    } else {
                        quote! {
                            format!("{:x?}", self.cur_inter.#name)
                        }
                    }
                } else {
                    quote! {
                        format!("{:#x?}", self.cur_inter.#name)
                    }
                };

                quote! {
                    info[#stage_index].signals.push((
                        stringify!(#name).to_string(),
                        #formatted_val
                    ));
                }
            })
            .reduce(|a, b| quote! { #a #b })
            .unwrap_or_default();

        quote! {
            #[allow(unused)]
            #[allow(non_snake_case)]
            fn get_stage_info(&self) -> Vec<crate::framework::StageInfo> {
                let mut info = vec![#stage_items];
                #sig_stmts
                info
            }
        }
    }

    fn render(&self) -> proc_macro2::TokenStream {
        let hardware = &self.hardware;
        let use_stmts = self
            .use_items
            .iter()
            .map(|item| item.to_token_stream())
            .reduce(|a, b| quote! { #a #b })
            .unwrap_or_default();

        let intermediate_signal_struct = self.render_intermediate_signal_struct();
        let build_circuit_fn = self.render_build_circuit();
        let update_fn = self.render_update();
        let get_stage_info_fn = self.render_get_stage_info();
        let pc_name = &self.program_counter;
        let termination = &self.termination;

        quote! {
            use #hardware::*;
            #use_stmts

            #intermediate_signal_struct

            #[allow(unused)]
            pub struct Arch;

            impl crate::framework::CpuCircuit for Arch {
                type UnitIn = UnitInputSignal;
                type UnitOut = UnitOutputSignal;
                type Inter = IntermediateSignal;
                type StageState = PipeRegs;
            }

            impl crate::framework::CpuArch for Arch {
                type Units = Units;
                #build_circuit_fn
            }

            impl crate::framework::PipeSim<Arch> {
                #update_fn
            }
            impl std::fmt::Display for crate::framework::PipeSim<Arch> {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    write!(f, "{}", self.circuit.order)
                }
            }
            impl crate::framework::CpuSim for crate::framework::PipeSim<Arch> {
                fn initiate_next_cycle(&mut self) {
                    self.cur_state.mux(&self.nex_state);
                }
                fn propagate_signals(&mut self) {
                    self.update();
                    self.cycle_count += 1;

                    if self.cur_inter.#termination {
                        self.terminate = true;
                    }
                }
                fn program_counter(&self) -> u64 {
                    self.cur_inter.#pc_name
                }
                fn is_terminate(&self) -> bool {
                    self.terminate
                }
                fn cycle_count(&self) -> u64 {
                    self.cycle_count
                }
                fn cycle_cost(&self) -> u64 {
                    self.circuit.order.max_dist as u64
                }
                fn registers(&self) -> crate::isa::RegFile {
                    use crate::framework::HardwareUnits;
                    self.units.register_file()
                }

                #get_stage_info_fn

                fn step(&mut self) {
                    use binutils::clap::builder::styling::*;
                    let title_style = Style::new().bold();

                    if self.tty_out {
                        println!(
                            "{title_style}{summary:=^80}{title_style:#}",
                            summary = format!(
                                " [Cycle {} (*{})] ",
                                self.cycle_count() + 1,
                                self.cycle_cost()
                            ),
                        );
                    }

                    use crate::framework::CpuSim;
                    self.propagate_signals();

                    tracing::trace!("{:?}", self.get_stage_info());

                    if self.tty_out {
                        println!(
                            "{title_style}PC = {:#x}{title_style:#}",
                            self.program_counter(),
                        );

                        // print the information of intermediate signals
                        self.print_state();
                        println!("{}", self.units);
                        let stages = self.get_stage_info();
                        for stage in stages {
                            tracing::info!("{:-^70}", format!(" {} ", stage.name));
                            for (name, val) in stage.signals {
                                tracing::info!("{:<10} = {}", name, val);
                            }
                        }
                    }

                    if self.is_terminate() {
                        if self.tty_out {
                            println!("terminate!");
                        }
                    } else {
                        self.initiate_next_cycle();
                    }
                }

                fn proporder(&self) -> &crate::framework::PropOrder {
                    &self.circuit.order
                }
            }
        }
    }
}

/// This macro parse the Hardware Control Language (HCL) introduced in CS:APP3e.
/// In general, it defines a set of signals, which connects outputs of units to
/// inputs of units through Boolean expressions.
#[proc_macro]
pub fn hcl(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let data: HclData = syn::parse(item).unwrap();
    data.render().into()
}

/// This macro can only be used in the `sim` crate.
#[proc_macro]
pub fn extra_pipelines(_item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR")
            .map(|s| s.to_string_lossy().to_string())
            .expect("CARGO_MANIFEST_DIR not found"),
    );
    assert!(dir.ends_with("sim"));

    let mut idents = Vec::new();

    let dir = dir.join("src/architectures/extra");
    for entry in std::fs::read_dir(dir).unwrap().filter_map(Result::ok) {
        if entry.file_type().unwrap().is_file() && entry.file_name() != "mod.rs" {
            if let Some(mod_name) = entry
                .file_name()
                .to_string_lossy()
                .to_string()
                .strip_suffix(".rs")
            {
                let mod_ident = format_ident!("{}", mod_name);
                idents.push(mod_ident);
            }
        }
    }

    let mod_stmts = idents
        .iter()
        .map(|id| quote! {pub mod #id;})
        .reduce(|a, b| quote! { #a #b })
        .unwrap_or_default();

    let case_stmts = idents
        .iter()
        .map(|id| {
            let id_name = id.to_string();
            quote! {
                #id_name => Box::new(super::PipeSim::<#id::Arch>::new(memory, tty_out)),
            }
        })
        .reduce(|a, b| quote! { #a #b })
        .unwrap_or_default();

    let name_list = idents
        .iter()
        .map(|id| id.to_string())
        .map(|s| quote! { #s })
        .reduce(|a, b| quote! { #a, #b })
        .unwrap_or_default();

    let n_name = idents.len();

    quote! {
        #mod_stmts

        pub const ARCH_NAMES: [&'static str; #n_name] = [#name_list];

        pub fn create_sim(
            kind: String, memory: super::MemData, tty_out: bool
        ) -> Box<dyn super::CpuSim> {
            match kind.as_str() {
                #case_stmts
                _ => panic!("Unknown architecture: {}", kind),
            }
        }
    }
    .into()
}
