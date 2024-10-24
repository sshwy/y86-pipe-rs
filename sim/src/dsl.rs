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
/// 3. On receving input signals, functional units process its input signals and
///    update its output signals, while stage units just store the inputs.
/// 4. After all signals reaching their destinations, the cycle ends. The inputs
///    of stage units will become the starting signals of the next cycle.
#[macro_export]
macro_rules! define_units {
    ($(
        $(#[$att:meta])*
        $unit_name:ident $unit_short_name:ident {
            $(.input( $($(#[$input_att:meta])* $iname:ident : $itype:ty),* ))?
            $(.output( $($(#[$output_att:meta])* $oname:ident : $otype:ty),* ))?
            $($sname:ident : $stype:ty),* $(,)?
        } $($body:block)?
    )*) => {
        /// Input signals of units
        pub mod unit_in {
            #![allow(unused_imports)]
            use super::*;
            $(#[derive(Default, Debug, Clone)]
            #[cfg_attr(feature = "serde", derive(serde::Serialize))]
            #[allow(non_snake_case)]
            pub struct $unit_name {
                $($($(#[$input_att])* pub $iname: $itype, )*)?
            })*
        }
        /// Output signals of units
        pub mod unit_out {
            #![allow(unused_imports)]
            use super::*;
            $(#[derive(Default, Debug, Clone)]
            #[cfg_attr(feature = "serde", derive(serde::Serialize))]
            #[allow(non_snake_case)]
            pub struct $unit_name {
                $($($(#[$output_att])* pub $oname: $otype, )*)?
            })*
        }

        #[derive(Default, Debug, Clone)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize))]
        pub struct UnitInputSignal {
            $(pub $unit_short_name: unit_in::$unit_name),*
        }
        #[derive(Default, Debug, Clone)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize))]
        pub struct UnitOutputSignal {
            $(pub $unit_short_name: unit_out::$unit_name),*
        }
        /// A unit simulates a circuit in the CPU. It receives signals from
        /// the previous stage and outputs signals to the next stage.
        ///
        /// Units include stages and combinational logics.
        pub trait Unit {
            fn run(&mut self, signals: (&UnitInputSignal, &mut UnitOutputSignal));
        }

        $( #[allow(unused)]
        $(#[$att])*
        struct $unit_name {
            $(pub $sname: $stype ),*
        } )*

        $( impl $unit_name {
            #[allow(unused)]
            pub fn trigger(Self{ $( $sname ),* }: &mut Self,
                inputs: &unit_in::$unit_name,
                outputs: &mut unit_out::$unit_name,
            ) {
                let unit_in::$unit_name{$($( $iname, )*)? .. } = inputs.clone();
                let unit_out::$unit_name{$($( $oname, )*)? .. } = outputs;
                $($body)?
            }
        }
        impl Unit for $unit_name {
            #[allow(unused)]
            fn run(&mut self, (input, output): (&UnitInputSignal, &mut UnitOutputSignal)) {
                $unit_name::trigger(self, &input.$unit_short_name, &mut output.$unit_short_name)
            }
        })*

        pub struct Units {
            $( $unit_short_name: $unit_name, )*
        }
        impl Units {
            /// Execute this unit by processing the input signals and updating its output signals.
            #[allow(unused)]
            pub fn run(&mut self, name: &'static str, sigs: (&UnitInputSignal, &mut UnitOutputSignal)) {
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
        pub fn hardware_setup(builder: &mut $crate::framework::PropOrderBuilder) {
            $(
            builder.add_unit_node(stringify!($unit_short_name));
            $( $( builder.add_unit_input(stringify!($unit_short_name), stringify!($iname)); )* )?
            $( $( builder.add_unit_output(stringify!($unit_short_name), stringify!($oname)); )* )?
            )*
        }
    };
}

/// In [`sim_macro::hcl`], The `a in {...}` expression is transformed into
/// `mtc(a, [...])`.
pub(crate) fn mtc<T: Eq>(sig: T, choice: impl AsRef<[T]>) -> bool {
    for c in choice.as_ref() {
        if *c == sig {
            return true;
        }
    }
    false
}

#[macro_export]
macro_rules! define_stages {
    ($(
        $(#[$stage_att:meta])*
        $pr_name:ident $pr_short_name:ident {
            $($(#[$pr_att:meta])* $pname:ident : $ptype:ty = $pdefault:expr),*
        }
    )*) => {
        /// Signals stored in stage units
        pub mod unit_stage {
            #![allow(unused_imports)]
            use super::*;
            use $crate::isa::inst_code::*;
            use $crate::isa::reg_code::*;
            use $crate::isa::op_code::*;
            $(#[derive(Debug, Clone)]
            #[cfg_attr(feature = "serde", derive(serde::Serialize))]
            $(#[$stage_att])*
            #[allow(non_snake_case)]
            pub struct $pr_name {
                $(pub $pname: $ptype, )*
                /// A special input of the pipeline register. If `bubble` is true,
                /// during the rise edge of the clock, registers in this stage will
                /// be reset to their default values.
                pub bubble: bool,
                /// If `stall` is true, the registers in this stage will keep the
                /// same value as the previous cycle.
                pub stall: bool,
            }
            impl Default for $pr_name {
                fn default() -> Self {
                    Self {
                        $( $pname: $pdefault, )*
                        bubble: false,
                        stall: false,
                    }
                }
            }
            impl $pr_name {
                /// Select states based on `new.bubble` and `new.stall`.
                ///
                /// The result is stored in `self`.
                #[allow(unused)]
                pub fn mux(&mut self, new: &Self) {
                    // this block is executed at the end of the cycle
                    if new.bubble {
                        $( self.$pname = $pdefault; )*
                        if new.stall {
                            tracing::error!("bubble and stall at the same time");
                        }
                    } else if !new.stall {
                        // if not stalled, we update the output signals
                        // by its input signals computed in this cycle
                        $( self.$pname = new.$pname; )*
                    }
                    // (stall) otherwise we keep the old state
                    // the same as the previous cycle
                }
            })*
        }

        /// All pipeline registers (all stages).
        #[derive(Default, Debug, Clone)]
        pub struct PipeRegs {
            $(pub $pr_short_name: unit_stage::$pr_name),*
        }

        impl PipeRegs {
            /// Trigger all pipeline registers at the end of the cycle.
            #[allow(unused)]
            pub fn mux(&mut self, new: &PipeRegs) {
                $( self.$pr_short_name.mux(&new.$pr_short_name); )*
            }
        }
    };
}
