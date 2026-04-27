use crate::gekko::sr::Sr;

#[inline(always)]
pub fn msr<const OP: u32>(ctx: &mut crate::gamecube::GameCube, instr: crate::gekko::instruction::Instruction) {
    match OP {
        crate::gekko::lut::OP_MTMSR => {
            ctx.gekko.msr = crate::gekko::msr::Msr::from(ctx.gekko.read_gpr(instr.rs()));
        }
        crate::gekko::lut::OP_MFMSR => {
            ctx.gekko.write_gpr(instr.rd(), ctx.gekko.msr.raw());
        }
        crate::gekko::lut::OP_RFI => {
            const RFI_MSR_MASK: u32 = 0x0000_FF73;
            ctx.gekko.msr =
                crate::gekko::msr::Msr::from((ctx.gekko.msr.raw() & !RFI_MSR_MASK) | (ctx.gekko.spr.srr1 & RFI_MSR_MASK));
            ctx.gekko.nia = ctx.gekko.spr.srr0.value() << 2;
        }
        _ => todo!("MSR instruction with OP = {OP:#x}"),
    }
}

#[inline(always)]
pub fn spr<const OP: u32>(ctx: &mut crate::gamecube::GameCube, instr: crate::gekko::instruction::Instruction) {
    match OP {
        crate::gekko::lut::OP_MTSPR => {
            let spr_num = instr.spr_swapped();
            let val = ctx.gekko.read_gpr(instr.rs());
            match spr_num {
                22 => {
                    ctx.scheduler.cancel(crate::gekko::dec::underflow_handler);
                    ctx.gekko.dec.write(ctx.scheduler.cycles, val);
                    ctx.gekko.spr.dec = val;
                    ctx.scheduler.schedule_in(
                        crate::gekko::dec::cycles_until_underflow(val),
                        crate::gekko::dec::underflow_handler,
                    );
                    tracing::debug!(cycles = ctx.scheduler.cycles, value = val, "decrementer set");
                }
                284 => ctx.scheduler.set_timebase_lower(val),
                285 => ctx.scheduler.set_timebase_upper(val),
                923 => {
                    ctx.gekko.spr.dmal = crate::gekko::spr::DmaLower::from_raw(val);
                    if ctx.gekko.spr.dmal.trigger() {
                        ctx.mmio.process_locked_cache_dma(&ctx.gekko.spr.dmau, &ctx.gekko.spr.dmal);
                    }
                }
                _ => ctx.gekko.spr.write(spr_num, val),
            }
        }
        crate::gekko::lut::OP_MFSPR => {
            let spr_num = instr.spr_swapped();
            let val = match spr_num {
                22 => {
                    ctx.gekko.spr.dec = ctx.gekko.dec.read(ctx.scheduler.cycles);
                    ctx.gekko.spr.dec
                }
                268 => ctx.scheduler.timebase_lower(),
                269 => ctx.scheduler.timebase_upper(),
                _ => ctx.gekko.spr.read(spr_num),
            };
            ctx.gekko.write_gpr(instr.rd(), val);
        }
        _ => todo!("SPR instruction with OP = {OP:#x}"),
    }
}

#[inline(always)]
pub fn segment<const OP: u32>(ctx: &mut crate::gamecube::GameCube, instr: crate::gekko::instruction::Instruction) {
    match OP {
        crate::gekko::lut::OP_MTSR => {
            ctx.gekko.sr[instr.sr() as usize] = Sr::from_raw(ctx.gekko.read_gpr(instr.rs()));
        }
        crate::gekko::lut::OP_MFSR => {
            ctx.gekko.write_gpr(instr.rd(), ctx.gekko.sr[instr.sr() as usize].raw());
        }
        _ => todo!("Segment Register instruction with OP = {OP:#x}"),
    }
}

#[inline(always)]
pub fn mtsrin(ctx: &mut crate::gamecube::GameCube, instr: crate::gekko::instruction::Instruction) {
    let sr_idx = (ctx.gekko.read_gpr(instr.rb()) >> 28) as usize;
    ctx.gekko.sr[sr_idx] = Sr::from_raw(ctx.gekko.read_gpr(instr.rs()));
}

#[inline(always)]
pub fn mfsrin(ctx: &mut crate::gamecube::GameCube, instr: crate::gekko::instruction::Instruction) {
    let sr_idx = (ctx.gekko.read_gpr(instr.rb()) >> 28) as usize;
    ctx.gekko.write_gpr(instr.rd(), ctx.gekko.sr[sr_idx].raw());
}

#[inline(always)]
pub fn mftb(ctx: &mut crate::gamecube::GameCube, instr: crate::gekko::instruction::Instruction) {
    let tbr = instr.spr_swapped();
    let val = match tbr {
        268 => ctx.scheduler.timebase_lower(),
        269 => ctx.scheduler.timebase_upper(),
        _ => panic!("unknown TBR {tbr}"),
    };
    ctx.gekko.write_gpr(instr.rd(), val);
}

#[inline(always)]
pub fn twi(ctx: &mut crate::gamecube::GameCube, instr: crate::gekko::instruction::Instruction) {
    let a = ctx.gekko.read_gpr(instr.ra()) as i32;
    let simm = instr.simm();
    let to = instr.to();

    let trap = (to & 0x10 != 0 && a < simm)
        || (to & 0x08 != 0 && a > simm)
        || (to & 0x04 != 0 && a == simm)
        || (to & 0x02 != 0 && (a as u32) < (simm as u32))
        || (to & 0x01 != 0 && (a as u32) > (simm as u32));

    if trap {
        ctx.cause_trap_exception();
    }
}

#[inline(always)]
pub fn tw(ctx: &mut crate::gamecube::GameCube, instr: crate::gekko::instruction::Instruction) {
    let a = ctx.gekko.read_gpr(instr.ra()) as i32;
    let b = ctx.gekko.read_gpr(instr.rb()) as i32;
    let to = instr.to();

    let trap = (to & 0x10 != 0 && a < b)
        || (to & 0x08 != 0 && a > b)
        || (to & 0x04 != 0 && a == b)
        || (to & 0x02 != 0 && (a as u32) < (b as u32))
        || (to & 0x01 != 0 && (a as u32) > (b as u32));

    if trap {
        ctx.cause_trap_exception();
    }
}

#[inline(always)]
pub fn nop<const OP: u32>(_ctx: &mut crate::gamecube::GameCube, _instr: crate::gekko::instruction::Instruction) {}

#[inline(always)]
pub fn sc(ctx: &mut crate::gamecube::GameCube, _instr: crate::gekko::instruction::Instruction) {
    ctx.cause_syscall_interrupt();
}
