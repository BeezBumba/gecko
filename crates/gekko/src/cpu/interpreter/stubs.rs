macro_rules! stub {
    ($($name:ident),* $(,)?) => {
        $(
            #[rustfmt::skip]
            pub fn $name(
                _ctx: &mut crate::gekko::Gekko,
                _instr: crate::cpu::semantics::Instruction,
            ) {
                todo!(stringify!($name))
            }
        )*
    };
}

stub! {
    twi,
    sc, tw,
    mtsrin, mcrxr,
    lwbrx, lswx, lswi, mfsrin,
    stswx, stwbrx, stswi,
    lhbrx,
    eciwx, ecowx,
    sthbrx,
}
