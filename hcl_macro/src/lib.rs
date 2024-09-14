use expr::LValue;
use items::{SignalDef, SignalSourceExpr, SignalSwitch};
use quote::{format_ident, quote, ToTokens};
use syn::{parse::Parse, parse_quote, punctuated::Punctuated, Token};
mod expr;
mod items;

#[derive(Debug, Default)]
struct StageAlias(Vec<(syn::Ident, syn::Ident)>);

impl Parse for StageAlias {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let args = Punctuated::<syn::MetaNameValue, Token![,]>::parse_terminated(input)?;
        Ok(Self(
            args.iter()
                .map(|arg| {
                    let ident = arg.path.get_ident().unwrap();
                    let value = &arg.value;
                    let value: syn::Ident = parse_quote! { #value };
                    (ident.clone(), value)
                })
                .collect(),
        ))
    }
}

struct HclData {
    hardware: syn::ExprPath,
    /// (cur, pre)
    stage_alias: StageAlias,
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
                    let stage_alias = attr.parse_args::<StageAlias>().unwrap();

                    Some(stage_alias)
                } else {
                    None
                }
            })
            .unwrap_or_default();

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
            #[cfg_attr(feature = "webapp", derive(serde::Serialize))]
            pub struct IntermediateSignal {
                #signal_fields
            }
        }
    }
    fn render_build_circuit(&self) -> proc_macro2::TokenStream {
        let stage_stmts = self
            .stage_alias
            .0
            .iter()
            .map(|(cur, pre)| {
                quote! {
                    g.add_stage_output(concat!("o.", stringify!(#cur)), stringify!(#pre));
                }
            })
            .reduce(|a, b| quote! { #a #b })
            .unwrap_or_default();

        let stmts = self
            .intermediate_signals
            .iter()
            .map(|signal| {
                let name = &signal.name;
                let update_deps = signal.source.lvalues();

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

        let inter = &quote::format_ident!("c");
        let inter_names = self
            .intermediate_signals
            .iter()
            .map(|s| &s.name)
            .collect::<Vec<_>>();
        let updaters_stmt = self
            .intermediate_signals
            .iter()
            .map(|s| HclData::render_signal_updater(s, inter, &inter_names, &self.stage_alias))
            .reduce(|a, b| quote! { #a #b })
            .unwrap_or_default();

        quote! {
            pub(crate) fn build_circuit() -> crate::pipeline::PropCircuit<Arch> {
                use crate::pipeline::*;

                // cur: o, nex: i
                let order = {
                    let mut g = PropOrderBuilder::new("o", "i");
                    #stage_stmts
                    // hardware setup
                    hardware_setup(&mut g);
                    #stmts
                    g.build()
                };

                use crate::isa::inst_code::*;
                use crate::isa::reg_code::*;
                use crate::isa::op_code::*;

                let mut circuit = PropCircuit::new(order);
                #updaters_stmt
                circuit
            }
        }
    }
    fn render_signal_updater(
        signal: &SignalDef,
        inter: &syn::Ident,
        inter_names: &[&syn::Ident],
        stage_alias: &StageAlias,
    ) -> proc_macro2::TokenStream {
        let name = &signal.name;
        let stage_alias = &stage_alias.0;

        let mapper = |mut lv: LValue| -> LValue {
            if inter_names.contains(&&lv.0[0]) {
                lv.0.insert(0, inter.clone().into());
            } else if let Some((cur, _)) = stage_alias.iter().find(|(_, pre)| &lv.0[0] == pre) {
                lv.0[0] = cur.clone();
                lv.0.insert(0, format_ident!("o"));
            }
            lv
        };

        let source_stmts = match &signal.source {
            items::SignalSource::Switch(SignalSwitch(cases)) => {
                let case_stmts = cases.iter().map(|case| {
                    let cond = case.condition.clone().map(mapper);
                    let val = case.value.clone().map(mapper);
                    let tunnel_stmts = case
                        .tunnel.as_ref().cloned()
                        .map(|tunnel| {
                            quote! {
                                has_tunnel_input = true;
                                eprintln!("{}", ansi_term::Colour::Green.bold().paint(stringify!(#tunnel)));
                                tracer.trigger_tunnel(stringify!(#tunnel));
                            }
                        });

                    quote! {
                        if (u8::from(#cond)) != 0 {
                            c.#name = #val;
                            #tunnel_stmts
                        }
                    }
                }).reduce(|a, b| quote! { #a else #b }).unwrap_or_default();

                quote! {
                    #case_stmts
                }
            }
            items::SignalSource::Expr(SignalSourceExpr { tunnel, expr }) => {
                let expr = expr.clone().map(mapper);
                let tunnel_stmts = tunnel.as_ref().cloned().map(|tunnel| {
                    quote! {
                        has_tunnel_input = true;
                        eprintln!("{}", ansi_term::Colour::Green.bold().paint(stringify!(#tunnel)));
                        tracer.trigger_tunnel(stringify!(#tunnel));
                    }
                });

                quote! {
                    c.#name = #expr;
                    #tunnel_stmts
                }
            }
        };

        let dest_tunnel_stmts = signal
            .destinations
            .iter()
            .map(|dest| {
                let dst = &dest.dest;
                if let Some(tunnel) = dest.tunnel.as_ref() {
                    quote! {
                        #dst = c.#name.to_owned();
                        if has_tunnel_input {
                            eprintln!("{}", ansi_term::Colour::Green.bold().paint(stringify!(#tunnel)));
                            tracer.trigger_tunnel(stringify!(#tunnel));
                        }
                    }
                } else {
                    quote! {
                        #dst = c.#name.to_owned();
                    }
                }
            })
            .reduce(|a, b| quote! { #a #b })
            .unwrap_or_default();

        quote! {
            {
                fn updater(
                    i: &mut UnitInputSignal,
                    c: &mut IntermediateSignal,
                    tracer: &mut Tracer,
                    o: UnitOutputSignal,
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
            #[allow(unused)]
            #[allow(non_snake_case)]
            fn update(&mut self) -> (UnitOutputSignal, crate::pipeline::Tracer) {
                let c = &mut self.cur_inter;
                let i = &mut self.cur_unit_in;
                let o = self.cur_unit_out.clone();

                let mut rcd = self.circuit.updates.make_propagator(i, o, c);
                let units = &mut self.units;
                for (is_unit, name) in &self.circuit.order.order {
                    if *is_unit {
                        let (mut unit_in, mut unit_out) = rcd.signals();
                        units.run(name, (unit_in, &mut unit_out));
                        rcd.update_from_unit_out(unit_out)
                    } else { // combinatorial logics do not change output (cur)
                        rcd.run_combinatorial_logic(name);
                    }
                }
                rcd.finalize()
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

        quote! {
            use #hardware::*;
            #use_stmts

            #intermediate_signal_struct

            #[allow(unused)]
            pub struct Arch;

            impl crate::pipeline::CpuCircuit for Arch {
                type UnitIn = UnitInputSignal;
                type UnitOut = UnitOutputSignal;
                type Inter = IntermediateSignal;
            }

            impl crate::pipeline::CpuArch for Arch {
                type Units = Units;
            }

            impl crate::pipeline::Pipeline<Arch> {
                #build_circuit_fn
                #update_fn

                pub fn new(units: Units) -> Self {
                    Self {
                        circuit: Self::build_circuit(),
                        cur_inter: IntermediateSignal::default(),
                        cur_unit_in: UnitInputSignal::default(),
                        cur_unit_out: UnitOutputSignal::default(),
                        units,
                        terminate: false,
                    }
                }
            }
        }
    }
}

#[proc_macro]
pub fn hcl(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let data: HclData = syn::parse(item).unwrap();
    data.render().into()
}
