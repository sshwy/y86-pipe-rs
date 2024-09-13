mod asm;
mod isa;
mod object;
mod pipeline;
mod record;
mod utils;

#[cfg(feature = "webapp")]
mod webapp;

pub use asm::assemble;
pub use asm::AssembleOption;
pub type DefaultPipeline = pipeline::Pipeline<pipeline::pipe_full::Signals, pipeline::hardware::Units>;
pub use utils::{mem_diff, mem_print};

/// This macro helps defining a set of devices composing a CPU.
#[macro_export]
macro_rules! define_units {
    ($(
        $(#[$att:meta])*
        $unit_name:ident $unit_short_name:ident {
        $(.input( $($iname:ident : $itype:ty),* ))?
        $(.output( $($oname:ident : $otype:ty),* ))?
        $(.pass( $($pname:ident : $ptype:ty = $pdefault:expr),* ))?$([$pvar:ident])?
        $($sname:ident : $stype:ty),* $(,)?
    } $($body:block)?)*) => {
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
            impl Default for $unit_name { // initialized as bubble status
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
                // $( let $pvar = unit_pass::$unit_name::load_default(); )?

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

        /// This function add all devices nodes, input ports, output ports and pass signals
        /// to the graph builder.
        pub fn hardware_setup(rcd: &mut $crate::record::GraphBuilder) {
            $(
               rcd.add_unit_node(stringify!($unit_short_name));
               $( $( rcd.add_unit_input(stringify!($unit_short_name), concat!(stringify!($unit_short_name), ".", stringify!($iname))); )* )?
               $( $( rcd.add_unit_output(stringify!($unit_short_name), concat!(stringify!($unit_short_name), ".", stringify!($oname))); )* )?
               $( $( rcd.add_unit_pass(stringify!($unit_short_name), stringify!($pname)); )* )?
            )*
        }
    };
}

fn mtc<T: Eq>(sig: T, choice: impl AsRef<[T]>) -> bool {
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
        @abbr $fstage:ident $dstage:ident $estage:ident $mstage:ident $wstage:ident

        $( @use $uty:path; )*

        // intermediate values
        $(
            // output intermediate value name and type
            $oname:ident $oty:ty

            // select from cases
            $(= [
                // can have multiple tunnel to trigger
                $($cond:expr => $val:expr; $( @$tun:ident )*)*
            ])?
            $(:= $final:expr $(, @$tun_final:ident )?)?
            $(=> $to:expr $(, @$tun_to:ident )?
            )*
            ;
        )*

        // tunnel visualizations. computations are performed at the end of cycle
        // before stage register update
        $(
            @tunnel $id:literal
            // [$start:expr] -> [$inter_or_end:expr]
            // $(-> [$end:expr] )?
            $(if $tunnel_cond:expr)?;
        )*
    } => {
        #[allow(unused_imports)]
        $( use $uty; )*

        #[derive(Debug, Default, Clone)]
        #[allow(unused)]
        #[cfg_attr(feature = "webapp", derive(serde::Serialize))]
        pub struct IntermediateSignal {
            $( pub $oname: $oty, )*
        }

        use $hardware::*;

        #[allow(unused)]
        pub type Signals = (UnitInputSignal, UnitOutputSignal, IntermediateSignal);

    impl $crate::pipeline::Pipeline<Signals, Units> {
        fn build_graph() -> $crate::record::Graph {
            let mut g = $crate::record::GraphBuilder::new(stringify!($cur), stringify!($nex));
            g.add_pass_output(concat!(stringify!($cur), ".f"), stringify!($fstage));
            g.add_pass_output(concat!(stringify!($cur), ".d"), stringify!($dstage));
            g.add_pass_output(concat!(stringify!($cur), ".e"), stringify!($estage));
            g.add_pass_output(concat!(stringify!($cur), ".m"), stringify!($mstage));
            g.add_pass_output(concat!(stringify!($cur), ".w"), stringify!($wstage));

            // hardware setup
            hardware_setup(&mut g);

            $(
                $(
                    g.add_update(
                        stringify!($oname), concat!($( concat!(
                            stringify!($cond), ";",
                            stringify!($val), ";",
                        ) ),*),
                    );
                )?
                $(
                    g.add_update(
                        stringify!($oname), stringify!($final),
                    );
                )?
                $( g.add_rev_deps(stringify!( $oname ), stringify!( $to )); )*
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
            use $crate::mtc;
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
                    $(if ($cond) as u8 != 0 {
                        $inter.$oname = $val;
                        $(
                            has_tunnel_input = true;
                            eprintln!("{}", ansi_term::Colour::Green.bold().paint(stringify!($tun)));
                            tracer.trigger_tunnel(stringify!($tun));
                        )*
                    })else*
                )?
                $( $inter.$oname = $final;
                    $(
                        eprintln!("{}", ansi_term::Colour::Blue.bold().paint(stringify!($tun_final)));
                        tracer.trigger_tunnel(stringify!($tun_final));
                        has_tunnel_input = true;
                    )?
                )?
                $(
                    $to = $inter.$oname.to_owned();
                    if has_tunnel_input {
                        $( eprintln!("{}", ansi_term::Colour::Blue.bold().paint(stringify!($tun_to)));
                        tracer.trigger_tunnel(stringify!($tun_to));)?
                    }
                )*
            };
            rcd.add_update(stringify!($oname), &mut updater); )*

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

#[cfg(test)]
mod tests {
    use crate::{assemble, isa::BIN_SIZE, AssembleOption};

    #[test]
    fn test_assemble() {
        let r = assemble(crate::asm::tests::RSUM_YS, AssembleOption::default()).unwrap();
        dbg!(&r.source);
        eprintln!("{}", r);
    }

    #[test]
    fn test_array() {
        let a: [u8; 65536] = [0; BIN_SIZE];
        let mut b = a;
        let c = a;
        b[0] = 12;
        eprintln!("{:?}, {:?}", b[0], c[0]);
    }
    /// in visualization of the architecture of pipeline, each tunnel
    /// starts from one ore more start points, may split to multiple heads,
    /// reaching various destination. What we concern is
    ///
    /// 1. whether the signal in this tunnel counts,
    /// 2. and what destination of it is important.
    ///
    /// The first one is determined by the source of its value.
    /// The second one is determined by the destination of the tunnel.
    ///
    /// To better define the visulization of tunnels, we can specify
    /// the sources and destinations that need to be visualized.
    /// Also some intermediate values are not visualized,
    /// but they are useful to determine whether a value counts.
    ///
    /// Design: available edges are:
    /// 1. unit output -> intermediate value
    /// 2. intermediate value -> unit input / intermediate value
    ///
    /// A tunnel can either be a single edge or two sets of
    /// edges (A, B), where the destination of A is just the source of B.
    ///
    /// Notices that the intermediate value only choose one from sources,
    /// and during visualization, a tunnel has a single source.
    /// Thus (A, B) can be reduced to (a -> c, B).
    ///
    /// We can first define the condition for each edge,
    /// and define tunnels explicitly. tunnel merging can be made
    /// automatically.
    ///
    /// Moreover, a tunnel is simply (source, intermediate, ...dist)
    /// For better readability, we maintain the condition separately.
    #[test]
    fn test_draw() {
        println!(
            r#"
                     ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
                     ┃      ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓┃
W stat icode       valE   valM      dstE dstM              ┃┃
   │     │           ┃      ┣━━━━━━━━│━━━━│━━━━━━━━━━━━━━━┓┃┃
   │     ├───#Mem.##┄┃┄┄┄┄┄Data##    │    │               ┃┃┃ 
   │     ├───Control┄┃┄┄┄┄┄memory    │    │               ┃┃┃
   │     │           ┃  Addr┛  ┃     │    │               ┃┃┃
   │     │           ┃  ┃ ┗━━━━┃━━━━━│━━━━│━━━━━━━━━━━━━━┓┃┃┃
   │     │           ┗━━╋━━━━━━┃━━━━━│━━━━│━━━━━━━━━━━━━┓┃┃┃┃
M stat icode    Cnd   valE   valA   dstE dstM           ┃┃┃┃┃
   │     │       │      ┣━━━━━━┃━━━━━│━━━━│━━━━━━━━━━━━┓┃┃┃┃┃
   │     │       CC─────ALU ┏━━┛     │    │            ┃┃┃┃┃┃
   │     │          AluA┛ ┗━┃━━AluB  │    │            ┃┃┃┃┃┃
   │     │           ┃┗━━━━━┫    ┃   │    │            ┃┃┃┃┃┃
E stat icode   ifun valC  valA valB dstE dstM srcA srcB┃┃┃┃┃┃
   │     │       │   ┃      ┃    ┃                     ┃┃┃┃┃┃
   │     │       │   ┃  #######━###━━━━━━━━━━━━━━━━━━━━┛┃┃┃┃┃
   │     │       │   ┃  #######━###━━━━━━━━━━━━━━━━━━━━━┛┃┃┃┃
   │     │       │   ┃  Sel+Fwd━Fwd━━━━━━━━━━━━━━━━━━━━━━┃┛┃┃
   │     │       │   ┃  ###A###━#B#━━━━━━━━━━━━━━━━━━━━━━┃━┫┃
   │     │       │   ┃  #######━###━━━━━━━━━━━━━━━━━━━━━━┃━┃┫
   │     │       │   ┃      ┃ ┃  ┃                       ┃ ┃┃
   │     │       │   ┃      ┃ ┗Register━━━━━━━━━━━━━━━━━━┃━┫┃
   │     │       │   ┃      ┗┓ ##file##━━━━━━━━━━━━━━━━━━┃━┃┛
   │     │       │   ┗━━━━━┓ ┗━━━┓                       ┃ ┃
D stat icode   ifun rA rB valC  valP                     ┃ ┃
   │     │       │   │ │   ┣━━━━━┃━━━━━━━━━━Predict      ┃ ┃
  Stat───┴───┐   │   │ │   ┃     ┣━━━━━━━━━━##PC###      ┃ ┃
             Instruction━━━┛  ###PC####        ┃         ┃ ┃
             ##memory###      increment        ┃         ┃ ┃
                  ┣━━━━━━━━━━━━━━┛             ┃         ┃ ┃
                Select━━━━━━━━━━━━━━━━━━━━━━━━━┃━━━━━━━━━┛ ┃
                ##PC##━━━━━━━━━━━━━━━━━━━━━━━━━┃━━━━━━━━━━━┛
F        predPC━┛                              ┃
            ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
"#
        )
    }
}
