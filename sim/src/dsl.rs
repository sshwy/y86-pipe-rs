/// This macro helps defining a set of devices composing a CPU.
///
/// There are two types of units: stage unit and functional unit.
///
/// During a CPU cycle,
/// 1. Signals in stage units, which is the result of the previous cycle, are
///    provided for this cycle.
/// 2. Signals start from the outputs of a functional unit (or stage unit), go
///    through pipes and combinational logics, finally reach the inputs of the
///    next functional unit or stage unit (stage units have inputs!).
/// 3. On receving input signals, functional units process its input signals
///    and update its output signals, while stage units just store the inputs.
/// 4. After all signals reaching their destinations, the cycle ends. The inputs
///    of stage units will become the starting signals of the next cycle.
///
/// **WARNING**: Do not use `.stage` with `.output` in the same unit. The
/// behavior is undefined.
#[macro_export]
macro_rules! define_units {
    ($(
        $(#[$att:meta])*
        $unit_name:ident $unit_short_name:ident {
            $(.input( $($iname:ident : $itype:ty),* ))?
            $(.output( $($oname:ident : $otype:ty),* ))?
            $(.stage( $($pname:ident : $ptype:ty = $pdefault:expr),* ))?
            $($sname:ident : $stype:ty),* $(,)?
        } $($body:block)?
    )*) => {
        pub mod unit_sig_in {
            #![allow(unused_imports)]
            use super::*;
            $(#[derive(Default, Debug, Clone)]
            #[cfg_attr(feature = "webapp", derive(serde::Serialize))]
            pub struct $unit_name {
                $($(pub $iname: $itype, )*)?
                $($(pub $pname: $ptype, )*)?
            })*
        }
        pub mod unit_sig_out {
            #![allow(unused_imports)]
            use super::*;
            $(#[derive(Debug, Clone)]
            #[cfg_attr(feature = "webapp", derive(serde::Serialize))]
            pub struct $unit_name {
                $($(pub $oname: $otype, )*)?
                $($(pub $pname: $ptype, )*)?
            }
            impl Default for $unit_name {
                fn default() -> Self {
                    Self {
                        $($($oname: Default::default(), )*)?
                        $($($pname: $pdefault, )*)?
                    }
                }
            })*
        }
        #[derive(Default, Debug, Clone)]
        #[cfg_attr(feature = "webapp", derive(serde::Serialize))]
        pub struct UnitInputSignal {
            $(pub $unit_short_name: unit_sig_in::$unit_name),*
        }
        #[derive(Default, Debug, Clone)]
        #[cfg_attr(feature = "webapp", derive(serde::Serialize))]
        pub struct UnitOutputSignal {
            $(pub $unit_short_name: unit_sig_out::$unit_name),*
        }

        /// A unit simulates a circuit in the CPU. It receives signals from
        /// the previous stage and outputs signals to the next stage.
        ///
        /// Units include stages and combinational logics.
        pub trait Unit {
            fn run(&mut self, signals: (UnitInputSignal, &mut UnitOutputSignal));
        }

        $( #[allow(unused)]
        $(#[$att])*
        struct $unit_name {
            $(pub $sname: $stype ),*
        } )*

        $( impl $unit_name {
            #[allow(unused)]
            pub fn trigger(Self{ $( $sname ),* }: &mut Self,
                inputs: unit_sig_in::$unit_name,
                outputs: &mut unit_sig_out::$unit_name,
            ) {
                let unit_sig_in::$unit_name{$($( $iname, )*)? .. } = inputs;
                let unit_sig_out::$unit_name{$($( $oname, )*)? .. } = outputs;

                $(
                    if inputs.bubble {
                        $( outputs.$pname = $pdefault; )*
                        if inputs.stall {
                            panic!("bubble and stall at the same time")
                        }
                    } else if !inputs.stall {
                        $( outputs.$pname = inputs.$pname; )*
                    } else { // stall
                        // do nothing
                    }
                )?

                // for functional units, we execute its logic here
                $($body)?
            }
        }
        impl Unit for $unit_name {
            #[allow(unused)]
            fn run(&mut self, (input, output): (UnitInputSignal, &mut UnitOutputSignal)) {
                $unit_name::trigger(self, input.$unit_short_name, &mut output.$unit_short_name)
            }
        }
        )*

        pub struct Units {
            $( $unit_short_name: $unit_name, )*
        }
        impl Units {
            /// Execute this unit by processing the input signals and updating its output signals.
            #[allow(unused)]
            pub fn run(&mut self, name: &'static str, sigs: (UnitInputSignal, &mut UnitOutputSignal)) {
                match name {
                    $( stringify!($unit_short_name) =>
                        self.$unit_short_name.run(sigs),
                    )*
                    _ => panic!("invalid name")
                }
            }
        }

        /// This function add all devices nodes, input ports, output ports and stage signals
        /// to the graph builder.
        pub fn hardware_setup(builder: &mut $crate::record::GraphBuilder) {
            $(
            builder.add_unit_node(stringify!($unit_short_name));
            $( $( builder.add_unit_input(stringify!($unit_short_name), stringify!($iname)); )* )?
            $( $( builder.add_unit_output(stringify!($unit_short_name), stringify!($oname)); )* )?
            $( $( builder.add_unit_stage(stringify!($unit_short_name), stringify!($pname)); )* )?
            )*
        }
    };
}

pub(crate) fn mtc<T: Eq>(sig: T, choice: impl AsRef<[T]>) -> bool {
    for c in choice.as_ref() {
        if *c == sig {
            return true;
        }
    }
    false
}

/// This macro minics the HCL language syntax to define hardware control logic.
/// See `pipe_full.rs` for an example
#[macro_export]
macro_rules! hcl {
    {
        // heads
        @hardware $hardware:path;
        @unit_input $nex:ident;
        @unit_output $cur:ident;
        @intermediate $inter:ident;
        @abbr $fstage:ident $dstage:ident $estage:ident $mstage:ident $wstage:ident;

        $( @use $uty:path; )*

        // intermediate values
        $(
            // output intermediate value name and type
            $cvar:ident $cty:ty

            // select from cases
            $(= [
                // can have multiple tunnel to trigger
                $($cond:expr => $val:expr; $( @$tun:ident )*)*
            ])?
            $(:= $final:expr $(, @$tun_final:ident )?)?
            $(=> $to:expr $(, @$tun_to:ident )?)*
            ;
        )*

        // tunnel visualizations. computations are performed at the end of cycle
        // before stage register update
        $(@tunnel $id:literal)*
    } => {
        #[allow(unused_imports)]
        $( use $uty; )*

        #[derive(Debug, Default, Clone)]
        #[allow(unused)]
        #[cfg_attr(feature = "webapp", derive(serde::Serialize))]
        pub struct IntermediateSignal {
            $( pub $cvar: $cty, )*
        }

        use $hardware::*;

        #[allow(unused)]
        pub type Signals = (UnitInputSignal, UnitOutputSignal, IntermediateSignal);

        impl $crate::pipeline::Pipeline<Signals, Units> {
            fn build_graph() -> $crate::record::Graph {
                let mut g = $crate::record::GraphBuilder::new(stringify!($cur), stringify!($nex));
                g.add_stage_output(concat!(stringify!($cur), ".f"), stringify!($fstage));
                g.add_stage_output(concat!(stringify!($cur), ".d"), stringify!($dstage));
                g.add_stage_output(concat!(stringify!($cur), ".e"), stringify!($estage));
                g.add_stage_output(concat!(stringify!($cur), ".m"), stringify!($mstage));
                g.add_stage_output(concat!(stringify!($cur), ".w"), stringify!($wstage));

                // hardware setup
                hardware_setup(&mut g);

                $(
                    $(
                        g.add_update(
                            stringify!($cvar), concat!($( concat!(
                                stringify!($cond), ";",
                                stringify!($val), ";",
                            ) ),*),
                        );
                    )?
                    $(
                        g.add_update(
                            stringify!($cvar), stringify!($final),
                        );
                    )?
                    $( g.add_rev_deps(stringify!( $cvar ), stringify!( $to )); )*
                )*

                g.build()
            }
            #[allow(unused)]
            #[allow(non_snake_case)]
            fn update(&mut self) -> (UnitOutputSignal, $crate::record::Tracer) {
                let $inter = &mut self.runtime_signals.2;
                let $nex = &mut self.runtime_signals.0;
                let $cur = self.runtime_signals.1.clone();
                let units = &mut self.units;

                use $crate::isa::inst_code::*;
                use $crate::isa::reg_code::*;
                use $crate::isa::op_code::*;
                use $crate::dsl::mtc;
                let $fstage = $cur.f.clone();
                let $dstage = $cur.d.clone();
                let $estage = $cur.e.clone();
                let $mstage = $cur.m.clone();
                let $wstage = $cur.w.clone();

                use $crate::record::*;
                let mut rcd = Record::new($nex, $cur, $inter);

                $( let mut updater = |
                    $nex: &mut UnitInputSignal,
                    $inter: &mut IntermediateSignal,
                    tracer: &mut Tracer,
                    $cur: UnitOutputSignal,
                | {
                    let mut has_tunnel_input = false;
                    $(
                        $(if (u8::from($cond)) != 0 {
                            $inter.$cvar = $val;
                            $(
                                has_tunnel_input = true;
                                eprintln!("{}", ansi_term::Colour::Green.bold().paint(stringify!($tun)));
                                tracer.trigger_tunnel(stringify!($tun));
                            )*
                        })else*
                    )?
                    $( $inter.$cvar = $final;
                        $(
                            eprintln!("{}", ansi_term::Colour::Blue.bold().paint(stringify!($tun_final)));
                            tracer.trigger_tunnel(stringify!($tun_final));
                            has_tunnel_input = true;
                        )?
                    )?
                    $(
                        $to = $inter.$cvar.to_owned();
                        if has_tunnel_input {
                            $( eprintln!("{}", ansi_term::Colour::Blue.bold().paint(stringify!($tun_to)));
                            tracer.trigger_tunnel(stringify!($tun_to));)?
                        }
                    )*
                };
                rcd.add_update(stringify!($cvar), &mut updater); )*

                for (is_unit, name) in &self.graph.order {
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
    };
}
