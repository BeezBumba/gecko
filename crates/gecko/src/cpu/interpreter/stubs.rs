macro_rules! stub {
    ($($name:ident),* $(,)?) => {
        $(
            #[rustfmt::skip]
            #[inline(always)]
            pub fn $name(
                _ctx: &mut crate::gamecube::GameCube,
                _instr: crate::cpu::instruction::Instruction,
            ) {
                todo!(stringify!($name))
            }
        )*
    };
}

stub! {
    tw,
    mtsrin, mcrxr,
    lswx, lswi, mfsrin,
    stswx, stswi,
    eciwx, ecowx,
}
