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
/// **WARNING**:
/// 1. Do not use `.stage` with `.output` in the same unit. The
///    behavior is undefined.
/// 2. The parameters in `.stage` must be `bubble` and `stall`. In this implementation,
///    pipeline registers (flip-flop) are considered as a special unit that is trigger
///    at the end of the cycle.
#[macro_export]
macro_rules! define_units {
    ($(
        $(#[$att:meta])*
        $unit_name:ident $unit_short_name:ident {
            $(.input( $($(#[$input_att:meta])* $iname:ident : $itype:ty),* ))?
            $(.output( $($(#[$output_att:meta])* $oname:ident : $otype:ty),* ))?
            $(.stage( $($(#[$stage_att:meta])* $pname:ident : $ptype:ty = $pdefault:expr),* ))?
            $($sname:ident : $stype:ty),* $(,)?
        } $($body:block)?
    )*) => {
        /// Input signals of units
        pub mod unit_in {
            #![allow(unused_imports)]
            use super::*;
            $(#[derive(Default, Debug, Clone)]
            #[cfg_attr(feature = "serde", derive(serde::Serialize))]
            pub struct $unit_name {
                $($($(#[$input_att])* pub $iname: $itype, )*)?
                $($($(#[$stage_att])* pub $pname: $ptype, )*)?
            })*
        }
        /// Output signals of units
        pub mod unit_out {
            #![allow(unused_imports)]
            use super::*;
            $(#[derive(Debug, Clone)]
            #[cfg_attr(feature = "serde", derive(serde::Serialize))]
            pub struct $unit_name {
                $($($(#[$output_att])* pub $oname: $otype, )*)?
                $($($(#[$stage_att])* pub $pname: $ptype, )*)?
            }
            impl Default for $unit_name {
                fn default() -> Self {
                    Self {
                        $($($oname: Default::default(), )*)?
                        // default values for stage units are assigned for the first cycle
                        $($($pname: $pdefault, )*)?
                    }
                }
            })*
        }
        /// Signals stored in stage units
        pub mod unit_stage {
            #![allow(unused_imports)]
            use super::*;
            $(#[derive(Debug, Clone)]
            #[cfg_attr(feature = "serde", derive(serde::Serialize))]
            pub struct $unit_name {
                $($(pub $pname: $ptype, )*)?
            }
            impl From<&super::unit_out::$unit_name> for $unit_name {
                fn from(_value: &super::unit_out::$unit_name) -> Self {
                    Self { $($($pname: _value.$pname, )*)? }
                }
            }
            impl $unit_name {
                pub fn update_output(self, _value: &mut super::unit_out::$unit_name) {
                    $($(_value.$pname = self.$pname; )*)?
                }
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
        /// All pipeline registers (all stages).
        #[derive(Debug, Clone)]
        pub struct PipeRegs {
            $(pub $unit_short_name: unit_stage::$unit_name),*
        }
        impl From<&UnitOutputSignal> for PipeRegs {
            fn from(value: &UnitOutputSignal) -> Self {
                Self { $( $unit_short_name: (&value.$unit_short_name).into(), )* }
            }
        }
        impl PipeRegs {
            pub fn update_output(self, value: &mut UnitOutputSignal) {
                $(self.$unit_short_name.update_output(&mut value.$unit_short_name); )*
            }
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
                inputs: unit_in::$unit_name,
                outputs: &mut unit_out::$unit_name,
            ) {
                let unit_in::$unit_name{$($( $iname, )*)? .. } = inputs;
                let unit_out::$unit_name{$($( $oname, )*)? .. } = outputs;

                // this block is executed at the end of the cycle
                // todo: implement this logic in hardware
                $(
                    if inputs.bubble {
                        $( outputs.$pname = $pdefault; )*
                        if inputs.stall {
                            panic!("bubble and stall at the same time")
                        }
                    } else if !inputs.stall {
                        // if not stalled, we update the output signals
                        // by its input signals computed in this cycle
                        $( outputs.$pname = inputs.$pname; )*
                    } else { // stall
                        // otherwise we keep the output signals
                        // the same as the previous cycle
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
        pub fn hardware_setup(builder: &mut $crate::framework::PropOrderBuilder) {
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
