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

        // repeatly parse the rest of the input
        loop {
            let lookahead = input.lookahead1();
            if input.is_empty() {
                break;
            } else if lookahead.peek(Token![use]) {
                let item = input.parse::<syn::ItemUse>()?;
                use_items.push(item);
            } else {
                let item = input.parse::<items::SignalDef>()?;
                intermediate_signals.push(item);
            }
        }

        Ok(Self {
            stage_alias,
            hardware,
            program_counter,
            termination,
            use_items,
            intermediate_signals,
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
            #[allow(unused)]
            #[cfg_attr(feature = "serde", derive(serde::Serialize))]
            pub struct IntermediateSignal {
                #signal_fields
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

        let mapper = |mut lv: LValue| -> LValue {
            if inter_names.contains(&&lv.0[0]) {
                lv.0.insert(0, inter.clone().into());
            } else if let Some((cur, _)) = stage_alias.iter().find(|(_, pre)| &lv.0[0] == pre) {
                lv.0[0] = cur.clone();
                lv.0.insert(0, format_ident!("p_"));
            } else if let Some((cur, _)) = stage_alias.iter().find(|(cur, _)| &lv.0[0] == cur) {
                lv.0[0] = cur.clone();
                lv.0.insert(0, format_ident!("n_"));
            }
            lv
        };

        let updaters_stmt = self
            .intermediate_signals
            .iter()
            .map(|s| HclData::render_signal_updater(s, mapper))
            .reduce(|a, b| quote! { #a #b })
            .unwrap_or_default();

        let stmts = self
            .intermediate_signals
            .iter()
            .map(|signal| {
                let name = &signal.name;
                let update_deps = signal
                    .source
                    .lvalues()
                    .into_iter()
                    .map(mapper.clone())
                    .collect::<Punctuated<LValue, Token![,]>>();

                let update_stmts = quote! {
                    g.add_update(stringify!(#name), stringify!(#update_deps));
                };
                let rev_deps_stmts = signal
                    .destinations
                    .iter()
                    .map(move |dest| {
                        let dest = &dest.dest;
                        quote! {
                            g.add_rev_deps(stringify!(#name), stringify!(#dest));
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
    fn render_signal_updater(
        signal: &SignalDef,
        mapper: impl Fn(LValue) -> LValue + Clone,
    ) -> proc_macro2::TokenStream {
        let name = &signal.name;

        let source_stmts = match &signal.source {
            items::SignalSource::Switch(SignalSwitch(cases)) => {
                let case_stmts = cases
                    .iter()
                    .map(|case| {
                        let cond = case.condition.clone().map(mapper.clone());
                        let val = case.value.clone().map(mapper.clone());
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
                let expr = expr.clone().map(mapper.clone());
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
                let dst = dest.dest.clone().map(mapper.clone());
                if let Some(tunnel) = dest.tunnel.as_ref() {
                    quote! {
                        #dst = c_.#name.to_owned();
                        if has_tunnel_input {
                            tracing::debug!("tunnel triggered: {}", stringify!(#tunnel));
                            tracer.trigger_tunnel(stringify!(#tunnel));
                        }
                    }
                } else {
                    quote! {
                        #dst = c_.#name.to_owned();
                    }
                }
            })
            .reduce(|a, b| quote! { #a #b })
            .unwrap_or_default();

        quote! {
            {
                fn updater(
                    i: &mut UnitInputSignal,
                    c_: &mut IntermediateSignal,
                    n_: &mut PipeRegs,
                    tracer: &mut Tracer,
                    o: &UnitOutputSignal,
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
                for (is_unit, name) in &self.circuit.order.order {
                    if *is_unit {
                        rcd.run_unit(|unit_in, unit_out| {
                            units.run(name, (unit_in, unit_out));
                        });
                    } else { // combinatorial logics do not change output (cur)
                        rcd.run_combinatorial_logic(name);
                    }
                }
                let (out, tracer) = rcd.finalize();
                self.cur_unit_out = out;
                tracer
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
                pub fn step(&mut self) {
                    use crate::framework::CpuSim;
                    tracing::info!("{:=^74}", " Run Cycle ");
                    self.propagate_signals();

                    if self.tty_out {
                        self.print_state();
                    }

                    if self.is_terminate() {
                        if self.tty_out {
                            println!("terminate!");
                        }
                    } else {
                        self.initiate_next_cycle();
                    }
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
            }
        }
    }
}

/// This macro parse the Hardware Control Language (HCL) introduced in CS:APP3e.
/// In general, it defines a set of signals, which connects outputs of units to inputs of units
/// through Boolean expressions.
#[proc_macro]
pub fn hcl(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let data: HclData = syn::parse(item).unwrap();
    data.render().into()
}
