use crate::gekko::condition::ConditionField;
use crate::gekko::instruction::Instruction;
use crate::gekko::lut::*;
use crate::system::{System, SystemId};

#[inline(always)]
pub fn compare<const OP: u32, const SYSTEM: SystemId>(ctx: &mut System<SYSTEM>, instr: Instruction) {
    ctx.scheduler.cycles += crate::gekko::cycles::cycles_for_op(OP) as u64;
    let (lt, gt, eq) = match OP {
        OP_CMP => {
            let a = ctx.gekko.read_gpr(instr.ra()) as i32;
            let b = ctx.gekko.read_gpr(instr.rb()) as i32;
            (a < b, a > b, a == b)
        }
        OP_CMPI => {
            let a = ctx.gekko.read_gpr(instr.ra()) as i32;
            let b = instr.simm();
            (a < b, a > b, a == b)
        }
        OP_CMPL => {
            let a = ctx.gekko.read_gpr(instr.ra());
            let b = ctx.gekko.read_gpr(instr.rb());
            (a < b, a > b, a == b)
        }
        OP_CMPLI => {
            let a = ctx.gekko.read_gpr(instr.ra());
            let b = instr.uimm() as u32;
            (a < b, a > b, a == b)
        }
        _ => todo!("Compare instruction with OP = {OP:#x}"),
    };

    let field = ConditionField::new()
        .with_lt(lt)
        .with_gt(gt)
        .with_eq(eq)
        .with_so(ctx.gekko.spr.xer.summary_overflow());
    ctx.gekko.cr.set_field(instr.crfd(), field);
}
