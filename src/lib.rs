mod asm;
mod isa;
mod object;
mod pipeline;
mod record;

pub use asm::assemble;
pub use asm::AssembleOption;

/// this macro helps defining a set of devices composing a CPU
#[macro_export]
macro_rules! define_devices {
    ($(
        $(#[$att:meta])*
        $dev_name:ident $dev_short_name:ident {
        $(.input( $($iname:ident : $itype:ty),* ))?
        $(.output( $($oname:ident : $otype:ty),* ))?
        $(.pass( $($pname:ident : $ptype:ty = $pdefault:expr),* ))?$([$pvar:ident])?
        $($sname:ident : $stype:ty),* $(,)?
    } $($body:block)?)*) => {
        pub mod dev_sig_in {
            #![allow(unused_imports)]
            use super::*;
            $(#[derive(Default, Debug, Clone)]
            pub struct $dev_name {
                $($(pub $iname: $itype, )*)?
                $($(pub $pname: $ptype, )*)?
            })*
        }
        pub mod dev_sig_out {
            #![allow(unused_imports)]
            use super::*;
            $(#[derive(Debug, Clone)]
            pub struct $dev_name {
                $($(pub $oname: $otype, )*)?
                $($(pub $pname: $ptype, )*)?
            }
            impl Default for $dev_name { // initialized as bubble status
                fn default() -> Self {
                    Self {
                        $($($oname: Default::default(), )*)?
                        $($($pname: $pdefault, )*)?
                    }
                }
            })*
        }
        // pub mod dev_pass {
        //     #![allow(unused)]
        //     #![allow(unused_imports)]
        //     use super::*;
        //     $(#[derive(Default, Debug, Clone)]
        //     pub struct $dev_name {
        //         $($(pub $pname: $ptype, )*)?
        //     }
        //     impl $dev_name {
        //         pub fn load_input(input: dev_sig_in::$dev_name) -> Self {
        //             Self {
        //                 $($( $pname: input.$pname, )*)?
        //             }
        //         }
        //         pub fn load_output(output: dev_sig_out::$dev_name) -> Self {
        //             Self {
        //                 $($( $pname: output.$pname, )*)?
        //             }
        //         }
        //         pub fn load_default() -> Self {
        //             Self {
        //                 $($( $pname: $pdefault, )*)?
        //             }
        //         }
        //     })*
        // }
        #[derive(Default, Debug, Clone)]
        pub struct DeviceInputSignal {
            $(pub $dev_short_name: dev_sig_in::$dev_name),*
        }
        #[derive(Default, Debug, Clone)]
        pub struct DeviceOutputSignal {
            $(pub $dev_short_name: dev_sig_out::$dev_name),*
        }

        // the trait of these signals
        pub trait Device {
            fn run(&mut self, signals: (DeviceInputSignal, &mut DeviceOutputSignal));
        }

        $( #[allow(unused)]
        $(#[$att])*
        struct $dev_name {
             $(pub $sname: $stype ),*
        } )*

        $( impl $dev_name {
            #[allow(unused)]
            pub fn trigger(Self{ $( $sname ),* }: &mut Self,
                inputs: dev_sig_in::$dev_name,
                outputs: &mut dev_sig_out::$dev_name,
            ) {
                let dev_sig_in::$dev_name{$($( $iname, )*)? .. } = inputs;
                let dev_sig_out::$dev_name{$($( $oname, )*)? .. } = outputs;

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
                // $( let $pvar = dev_pass::$dev_name::load_default(); )?

                $($body)?
            }
        } 
        impl Device for $dev_name {
            #[allow(unused)]
            fn run(&mut self, (input, output): (DeviceInputSignal, &mut DeviceOutputSignal)) {
                $dev_name::trigger(self, input.$dev_short_name, &mut output.$dev_short_name)
            }
        }
        )*

        pub struct Devices {
            $( $dev_short_name: $dev_name, )*
        }
        impl Devices {
            #[allow(unused)]
            pub fn run_name(&mut self, name: &'static str, sigs: (DeviceInputSignal, &mut DeviceOutputSignal)) {
                match name {
                    $( stringify!($dev_short_name) =>
                        self.$dev_short_name.run(sigs),
                    )*
                    _ => panic!("invalid name")
                }
            }
        }

        pub fn hardware_setup<T>(rcd: &mut $crate::record::RecordBuilder<'_, DeviceInputSignal, DeviceOutputSignal, T>) {
            $(
               rcd.add_device_node(stringify!($dev_short_name));
               $( $( rcd.add_device_input(stringify!($dev_short_name), stringify!($iname)); )* )?
               $( $( rcd.add_device_output(stringify!($dev_short_name), stringify!($oname)); )* )?
               $( $( rcd.add_device_pass(stringify!($dev_short_name), stringify!($pname)); )* )?
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

// Define hardware control logic
#[macro_export]
macro_rules! hcl {
    {
        @hardware $hardware:path;
        @devinput $nex:ident;
        @devoutput $cur:ident;
        @intermediate $inter:ident;
        @abbr $fstage:ident $dstage:ident $estage:ident $mstage:ident $wstage:ident

        $( @use $uty:path; )*
        // @icodes $icodes:ident;

        $(
            $oname:ident $oty:ty
            $(= [ $($cond:expr => $val:expr;)* ])?
            $(:= $final:expr)?
            $(=> $to:expr)*
            ;
        )*
    } => {
        #[allow(unused_imports)]
        $( use $uty; )*

        #[derive(Debug, Default, Clone)]
        #[allow(unused)]
        pub struct IntermediateSignal {
            $( $oname: $oty, )*
        }

        use $hardware::*;
        use $crate::record::NameList;

        #[allow(unused)]
        pub type Signals = (DeviceInputSignal, DeviceOutputSignal, IntermediateSignal);

impl $crate::pipeline::Pipeline<Signals, Devices> {
        #[allow(unused)]
        #[allow(non_snake_case)]
        fn update(
            $inter: &mut IntermediateSignal,
            $nex: &mut DeviceInputSignal,
            $cur: DeviceOutputSignal,
            devices: &mut Devices,
            preserved_order: Option<NameList>,
        ) -> (DeviceOutputSignal, $crate::record::TransLog, NameList) {
            use $crate::isa::inst_code::*;
            use $crate::isa::reg_code::*;
            use $crate::isa::op_code::*;
            // use $crate::isa::cond_fn as COND;
            use $crate::mtc;
            let $fstage = $cur.f.clone();
            let $dstage = $cur.d.clone();
            let $estage = $cur.e.clone();
            let $mstage = $cur.m.clone();
            let $wstage = $cur.w.clone();

            let mut rcd = $crate::record::RecordBuilder::new(stringify!($cur), stringify!($nex));
            rcd.add_pass_output(concat!(stringify!($cur), ".f"), stringify!($fstage));
            rcd.add_pass_output(concat!(stringify!($cur), ".d"), stringify!($dstage));
            rcd.add_pass_output(concat!(stringify!($cur), ".e"), stringify!($estage));
            rcd.add_pass_output(concat!(stringify!($cur), ".m"), stringify!($mstage));
            rcd.add_pass_output(concat!(stringify!($cur), ".w"), stringify!($wstage));

            // hardware setup
            hardware_setup(&mut rcd);

            $(
                let mut $oname = |
                    $nex: &mut DeviceInputSignal,
                    $inter: &mut IntermediateSignal,
                    logs: &mut $crate::record::TransLog,
                    $cur: DeviceOutputSignal,
                | {
                    $(
                        $(if ($cond) as u8 != 0 {
                            $inter.$oname = $val;
                            logs.push(( stringify!($oname), stringify!($val) ))
                        })else*
                    )?
                    $( $inter.$oname = $final; )?
                    $( $to = $inter.$oname.to_owned(); )*
                };
                $(
                    rcd.add_update(
                        stringify!($oname), concat!($( concat!(
                            stringify!($cond), ";",
                            stringify!($val), ";",
                        ) ),*),
                        &mut $oname
                    );
                )?
                $(
                    rcd.add_update(
                        stringify!($oname), stringify!($final),
                        &mut $oname
                    );
                )?
                $( rcd.add_rev_deps(stringify!( $oname ), stringify!( $to )); )*
            )*

            // static PRESERVED_ORDER: std::cell::RefCell<> = None.into();


            let mut rcd = rcd.build($nex, $cur, $inter, preserved_order);

            let order = rcd.toporder();
            let mut logs = Vec::new();
            for (is_device, name) in order.clone() {
                if is_device {
                    let (mut devin, mut devout) = rcd.clone_devsigs();
                    devices.run_name(name, (devin, &mut devout));
                    rcd.update_devout(devout)
                } else { // combinatorial logics do not change output (cur)
                    rcd.run_name(name, &mut logs);
                }
            }
            // todo: register execution, status handling
            (rcd.clone_devsigs().1, logs, order)
        }

}
    };
}

#[cfg(test)]
mod tests {
    use crate::{assemble, AssembleOption};

    #[test]
    fn test_assemble() {
        let r = assemble(crate::asm::tests::RSUM_YS, AssembleOption::default()).unwrap();
        dbg!(&r.source);
        eprintln!("{}", r);
    }
}
