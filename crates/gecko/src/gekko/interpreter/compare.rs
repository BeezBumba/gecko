use crate::gekko::condition::ConditionRegister;

#[inline(always)]
pub fn compare<const OP: u32>(ctx: &mut crate::gamecube::GameCube, instr: crate::gekko::instruction::Instruction) {
    let field = match OP {
        crate::gekko::lut::OP_CMP => ConditionRegister::field_from_ord(
            (ctx.gekko.read_gpr(instr.ra()) as i32).cmp(&(ctx.gekko.read_gpr(instr.rb()) as i32)),
        ),
        crate::gekko::lut::OP_CMPI => {
            ConditionRegister::field_from_ord((ctx.gekko.read_gpr(instr.ra()) as i32).cmp(&instr.simm()))
        }
        crate::gekko::lut::OP_CMPL => {
            ConditionRegister::field_from_ord(ctx.gekko.read_gpr(instr.ra()).cmp(&ctx.gekko.read_gpr(instr.rb())))
        }
        crate::gekko::lut::OP_CMPLI => {
            ConditionRegister::field_from_ord(ctx.gekko.read_gpr(instr.ra()).cmp(&(instr.uimm() as u32)))
        }
        _ => todo!("Compare instruction with OP = {OP:#x}"),
    };

    ctx.gekko.cr.set_field(instr.crfd(), field);
}
