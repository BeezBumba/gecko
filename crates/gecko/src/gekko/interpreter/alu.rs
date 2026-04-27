#[inline(always)]
pub fn alu<const OP: u32>(ctx: &mut crate::gamecube::GameCube, instr: crate::gekko::instruction::Instruction) {
    match OP {
        crate::gekko::lut::OP_ADDX => {
            let res = ctx.gekko.read_gpr(instr.ra()).wrapping_add(ctx.gekko.read_gpr(instr.rb()));
            ctx.gekko.write_gpr(instr.rd(), res);
            if instr.rc() {
                ctx.gekko.update_cr0(res);
            }
        }
        crate::gekko::lut::OP_ADDI | crate::gekko::lut::OP_ADDIS => {
            let ra = ctx.gekko.read_gpr_or_zero(instr.ra());
            let simm = if OP == crate::gekko::lut::OP_ADDIS {
                instr.simm() << 16
            } else {
                instr.simm()
            };
            ctx.gekko.write_gpr(instr.rd(), ra.wrapping_add_signed(simm));
        }
        crate::gekko::lut::OP_ORI | crate::gekko::lut::OP_ORIS => {
            let imm = if OP == crate::gekko::lut::OP_ORIS {
                (instr.uimm() as u32) << 16
            } else {
                instr.uimm() as u32
            };
            ctx.gekko.write_gpr(instr.ra(), ctx.gekko.read_gpr(instr.rs()) | imm);
        }
        crate::gekko::lut::OP_XORI | crate::gekko::lut::OP_XORIS => {
            let imm = if OP == crate::gekko::lut::OP_XORIS {
                (instr.uimm() as u32) << 16
            } else {
                instr.uimm() as u32
            };
            ctx.gekko.write_gpr(instr.ra(), ctx.gekko.read_gpr(instr.rs()) ^ imm);
        }
        crate::gekko::lut::OP_ANDI_DOT | crate::gekko::lut::OP_ANDIS_DOT => {
            let mask = if OP == crate::gekko::lut::OP_ANDIS_DOT {
                (instr.uimm() as u32) << 16
            } else {
                instr.uimm() as u32
            };
            let val = ctx.gekko.read_gpr(instr.rs()) & mask;
            ctx.gekko.write_gpr(instr.ra(), val);
            ctx.gekko.update_cr0(val);
        }
        crate::gekko::lut::OP_SUBFX => {
            let res = ctx.gekko.read_gpr(instr.rb()).wrapping_sub(ctx.gekko.read_gpr(instr.ra()));
            ctx.gekko.write_gpr(instr.rd(), res);
            if instr.rc() {
                ctx.gekko.update_cr0(res);
            }
        }
        crate::gekko::lut::OP_NEGX => {
            let res = (!ctx.gekko.read_gpr(instr.ra())).wrapping_add(1);
            ctx.gekko.write_gpr(instr.rd(), res);
            if instr.rc() {
                ctx.gekko.update_cr0(res);
            }
        }
        crate::gekko::lut::OP_ADDCX => {
            let ra = ctx.gekko.read_gpr(instr.ra());
            let rb = ctx.gekko.read_gpr(instr.rb());
            let (res, carry) = ra.overflowing_add(rb);
            ctx.gekko.write_gpr(instr.rd(), res);
            ctx.gekko.set_xer_ca(carry);
            if instr.rc() {
                ctx.gekko.update_cr0(res);
            }
        }
        crate::gekko::lut::OP_SUBFCX => {
            let ra = ctx.gekko.read_gpr(instr.ra());
            let rb = ctx.gekko.read_gpr(instr.rb());
            let res = rb.wrapping_sub(ra);
            ctx.gekko.set_xer_ca(rb >= ra);
            ctx.gekko.write_gpr(instr.rd(), res);
            if instr.rc() {
                ctx.gekko.update_cr0(res);
            }
        }
        crate::gekko::lut::OP_ADDEX => {
            let ra = ctx.gekko.read_gpr(instr.ra());
            let rb = ctx.gekko.read_gpr(instr.rb());
            let ca = ctx.gekko.xer_ca();
            let (t1, c1) = ra.overflowing_add(rb);
            let (res, c2) = t1.overflowing_add(ca);
            ctx.gekko.write_gpr(instr.rd(), res);
            ctx.gekko.set_xer_ca(c1 || c2);
            if instr.rc() {
                ctx.gekko.update_cr0(res);
            }
        }
        crate::gekko::lut::OP_SUBFEX => {
            let ra = ctx.gekko.read_gpr(instr.ra());
            let rb = ctx.gekko.read_gpr(instr.rb());
            let ca = ctx.gekko.xer_ca();
            let (t1, c1) = (!ra).overflowing_add(rb);
            let (res, c2) = t1.overflowing_add(ca);
            ctx.gekko.write_gpr(instr.rd(), res);
            ctx.gekko.set_xer_ca(c1 || c2);
            if instr.rc() {
                ctx.gekko.update_cr0(res);
            }
        }
        crate::gekko::lut::OP_ADDZEX => {
            let ra = ctx.gekko.read_gpr(instr.ra());
            let ca = ctx.gekko.xer_ca();
            let (res, carry) = ra.overflowing_add(ca);
            ctx.gekko.write_gpr(instr.rd(), res);
            ctx.gekko.set_xer_ca(carry);
            if instr.rc() {
                ctx.gekko.update_cr0(res);
            }
        }
        crate::gekko::lut::OP_SUBFZEX => {
            let ra = ctx.gekko.read_gpr(instr.ra());
            let ca = ctx.gekko.xer_ca();
            let (res, carry) = (!ra).overflowing_add(ca);
            ctx.gekko.write_gpr(instr.rd(), res);
            ctx.gekko.set_xer_ca(carry);
            if instr.rc() {
                ctx.gekko.update_cr0(res);
            }
        }
        crate::gekko::lut::OP_ADDMEX => {
            let ra = ctx.gekko.read_gpr(instr.ra());
            let ca = ctx.gekko.xer_ca();
            let (t1, c1) = ra.overflowing_add(ca);
            let (res, c2) = t1.overflowing_add(0xFFFF_FFFF);
            ctx.gekko.write_gpr(instr.rd(), res);
            ctx.gekko.set_xer_ca(c1 || c2);
            if instr.rc() {
                ctx.gekko.update_cr0(res);
            }
        }
        crate::gekko::lut::OP_SUBFMEX => {
            let ra = ctx.gekko.read_gpr(instr.ra());
            let ca = ctx.gekko.xer_ca();
            let (t1, c1) = (!ra).overflowing_add(ca);
            let (res, c2) = t1.overflowing_add(0xFFFF_FFFF);
            ctx.gekko.write_gpr(instr.rd(), res);
            ctx.gekko.set_xer_ca(c1 || c2);
            if instr.rc() {
                ctx.gekko.update_cr0(res);
            }
        }
        crate::gekko::lut::OP_MULLWX => {
            let res = (ctx.gekko.read_gpr(instr.ra()) as i32 as i64)
                .wrapping_mul(ctx.gekko.read_gpr(instr.rb()) as i32 as i64) as u32;
            ctx.gekko.write_gpr(instr.rd(), res);
            if instr.rc() {
                ctx.gekko.update_cr0(res);
            }
        }
        crate::gekko::lut::OP_DIVWUX => {
            let ra = ctx.gekko.read_gpr(instr.ra());
            let rb = ctx.gekko.read_gpr(instr.rb());
            let res = if rb == 0 { 0 } else { ra / rb };
            ctx.gekko.write_gpr(instr.rd(), res);
            if instr.rc() {
                ctx.gekko.update_cr0(res);
            }
        }
        crate::gekko::lut::OP_DIVWX => {
            let ra = ctx.gekko.read_gpr(instr.ra()) as i32;
            let rb = ctx.gekko.read_gpr(instr.rb()) as i32;
            let res = if rb == 0 || (ra == i32::MIN && rb == -1) {
                0u32
            } else {
                (ra / rb) as u32
            };
            ctx.gekko.write_gpr(instr.rd(), res);
            if instr.rc() {
                ctx.gekko.update_cr0(res);
            }
        }
        crate::gekko::lut::OP_MULHWUX => {
            let res = ((ctx.gekko.read_gpr(instr.ra()) as u64 * ctx.gekko.read_gpr(instr.rb()) as u64) >> 32) as u32;
            ctx.gekko.write_gpr(instr.rd(), res);
            if instr.rc() {
                ctx.gekko.update_cr0(res);
            }
        }
        crate::gekko::lut::OP_MULHWX => {
            let res = ((ctx.gekko.read_gpr(instr.ra()) as i32 as i64 * ctx.gekko.read_gpr(instr.rb()) as i32 as i64) >> 32)
                as u32;
            ctx.gekko.write_gpr(instr.rd(), res);
            if instr.rc() {
                ctx.gekko.update_cr0(res);
            }
        }
        crate::gekko::lut::OP_SUBFIC => {
            let ra = ctx.gekko.read_gpr(instr.ra());
            let simm = instr.simm() as u32;
            ctx.gekko.write_gpr(instr.rd(), simm.wrapping_sub(ra));
            ctx.gekko.set_xer_ca(simm >= ra);
        }
        crate::gekko::lut::OP_ADDIC | crate::gekko::lut::OP_ADDIC_DOT => {
            let ra = ctx.gekko.read_gpr(instr.ra());
            let (res, carry) = ra.overflowing_add(instr.simm() as u32);
            ctx.gekko.write_gpr(instr.rd(), res);
            ctx.gekko.set_xer_ca(carry);
            if OP == crate::gekko::lut::OP_ADDIC_DOT {
                ctx.gekko.update_cr0(res);
            }
        }
        crate::gekko::lut::OP_MULLI => {
            let res = (ctx.gekko.read_gpr(instr.ra()) as i32 as i64).wrapping_mul(instr.simm() as i64) as u32;
            ctx.gekko.write_gpr(instr.rd(), res);
        }
        _ => todo!("ALU instruction with OP = {OP:#x}"),
    }
}

#[inline(always)]
pub fn logical<const OP: u32>(ctx: &mut crate::gamecube::GameCube, instr: crate::gekko::instruction::Instruction) {
    let rs = ctx.gekko.read_gpr(instr.rs());
    let rb = ctx.gekko.read_gpr(instr.rb());

    let res = match OP {
        crate::gekko::lut::OP_ANDX => rs & rb,
        crate::gekko::lut::OP_ORX => rs | rb,
        crate::gekko::lut::OP_XORX => rs ^ rb,
        crate::gekko::lut::OP_NANDX => !(rs & rb),
        crate::gekko::lut::OP_NORX => !(rs | rb),
        crate::gekko::lut::OP_ANDCX => rs & !rb,
        crate::gekko::lut::OP_ORCX => rs | !rb,
        crate::gekko::lut::OP_EQVX => !(rs ^ rb),
        crate::gekko::lut::OP_SLWX => {
            let sh = rb & 0x3F;
            if sh >= 32 { 0 } else { rs << sh }
        }
        crate::gekko::lut::OP_SRWX => {
            let sh = rb & 0x3F;
            if sh >= 32 { 0 } else { rs >> sh }
        }
        crate::gekko::lut::OP_SRAWX => {
            let sh = rb & 0x3F;
            let signed = rs as i32;
            if sh >= 32 {
                ctx.gekko.set_xer_ca(signed < 0);
                (signed >> 31) as u32
            } else if sh == 0 {
                ctx.gekko.set_xer_ca(false);
                rs
            } else {
                let mask = (1u32 << sh) - 1;
                ctx.gekko.set_xer_ca(signed < 0 && (rs & mask) != 0);
                (signed >> sh) as u32
            }
        }
        crate::gekko::lut::OP_SRAWIX => {
            let sh = instr.sh() as u32;
            let signed = rs as i32;
            if sh == 0 {
                ctx.gekko.set_xer_ca(false);
                rs
            } else {
                let mask = (1u32 << sh) - 1;
                ctx.gekko.set_xer_ca(signed < 0 && (rs & mask) != 0);
                (signed >> sh) as u32
            }
        }
        crate::gekko::lut::OP_CNTLZWX => rs.leading_zeros(),
        crate::gekko::lut::OP_EXTSHX => rs as i16 as i32 as u32,
        crate::gekko::lut::OP_EXTSBX => rs as i8 as i32 as u32,
        _ => todo!("Logical instruction with OP = {OP:#x}"),
    };

    ctx.gekko.write_gpr(instr.ra(), res);
    if instr.rc() {
        ctx.gekko.update_cr0(res);
    }
}
