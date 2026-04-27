#[inline]
fn mask(mb: u32, me: u32) -> u32 {
    let begin = 0xFFFF_FFFFu32 >> mb;
    let end = if me >= 31 { 0 } else { 0xFFFF_FFFFu32 >> (me + 1) };
    if mb <= me { begin & !end } else { begin | !end }
}

#[inline(always)]
pub fn rotate<const OP: u32>(ctx: &mut crate::gamecube::GameCube, instr: crate::gekko::instruction::Instruction) {
    let rs = ctx.gekko.read_gpr(instr.rs());
    let mb = instr.mb() as u32;
    let me = instr.me() as u32;

    let res = match OP {
        crate::gekko::lut::OP_RLWINMX => rs.rotate_left(instr.sh() as u32) & mask(mb, me),
        crate::gekko::lut::OP_RLWIMIX => {
            let m = mask(mb, me);
            let r = rs.rotate_left(instr.sh() as u32);
            (r & m) | (ctx.gekko.read_gpr(instr.ra()) & !m)
        }
        crate::gekko::lut::OP_RLWNMX => {
            let sh = ctx.gekko.read_gpr(instr.rb()) & 0x1F;
            rs.rotate_left(sh) & mask(mb, me)
        }
        _ => todo!("Rotate instruction with OP = {OP:#x}"),
    };

    ctx.gekko.write_gpr(instr.ra(), res);
    if instr.rc() {
        ctx.gekko.update_cr0(res);
    }
}
