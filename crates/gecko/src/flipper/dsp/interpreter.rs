use crate::flipper::dsp::{condition::BranchControl, core::{SignExtensionMode, StatusRegister}, lut::*};

#[inline(always)]
pub fn add_sub<const OP: u32>(
    _ctx: &mut crate::gamecube::GameCube,
    _instr: crate::flipper::dsp::instruction::Instruction,
) {
    match OP {
        OP_ADDR => todo!("addr"),
        OP_ADDAX => todo!("addax"),
        OP_ADD => todo!("add"),
        OP_ADDP => todo!("addp"),
        OP_SUBR => todo!("subr"),
        OP_SUBAX => todo!("subax"),
        OP_SUB => todo!("sub"),
        OP_SUBP => todo!("subp"),
        OP_ADDAXL => todo!("addaxl"),
        OP_ADDPAXZ => todo!("addpaxz"),
        _ => unreachable!(),
    }
}

#[inline(always)]
pub fn addr_reg<const OP: u32>(
    _ctx: &mut crate::gamecube::GameCube,
    _instr: crate::flipper::dsp::instruction::Instruction,
) {
    match OP {
        OP_DAR => todo!("dar"),
        OP_IAR => todo!("iar"),
        OP_SUBARN => todo!("subarn"),
        OP_ADDARN => todo!("addarn"),
        _ => unreachable!(),
    }
}

#[inline(always)]
pub fn cmp_test<const OP: u32>(
    _ctx: &mut crate::gamecube::GameCube,
    _instr: crate::flipper::dsp::instruction::Instruction,
) {
    match OP {
        OP_CMP => todo!("cmp"),
        OP_CMPAXH => todo!("cmpaxh"),
        OP_TST => todo!("tst"),
        OP_TSTPROD => todo!("tstprod"),
        OP_TSTAXH => todo!("tstaxh"),
        OP_NX_0 => todo!("nx_0"),
        OP_NX_1 => todo!("nx_1"),
        OP_CLR => todo!("clr"),
        OP_CLRP => todo!("clrp"),
        OP_CLRL => todo!("clrl"),
        _ => unreachable!(),
    }
}

#[inline(always)]
pub fn control<const OP: u32>(
    ctx: &mut crate::gamecube::GameCube,
    instr: crate::flipper::dsp::instruction::Instruction,
) {
    match OP {
        OP_JCC => {
            let branch_control = BranchControl::from(instr.cond());
            if branch_control.evaluate(&ctx.dsp) {
                ctx.dsp.registers.nia = instr.addr();
            }
        }
        OP_NOP => {},
        OP_HALT => {
            ctx.dsp.csr.set_halt(true);
        },
        OP_IFCC => todo!("ifcc"),
        OP_CALLCC => todo!("callcc"),
        OP_RETCC => todo!("retcc"),
        OP_RTICC => {
            let branch_control = BranchControl::from(instr.cond());
            if branch_control.evaluate(&ctx.dsp) {
                ctx.dsp.registers.status = StatusRegister::from(ctx.dsp.registers.data_stack.pop());
                ctx.dsp.registers.nia = ctx.dsp.registers.call_stack.pop();
            }
        },
        OP_JRCC => todo!("jrcc"),
        OP_CALLRCC => todo!("callrcc"),
        _ => unreachable!(),
    }
}

#[inline(always)]
pub fn imm_alu<const OP: u32>(
    ctx: &mut crate::gamecube::GameCube,
    instr: crate::flipper::dsp::instruction::Instruction,
) {
    match OP {
        OP_ADDI => todo!("addi"),
        OP_XORI => todo!("xori"),
        OP_ANDI => todo!("andi"),
        OP_ORI => todo!("ori"),
        OP_CMPI => todo!("cmpi"),
        OP_ANDF | OP_ANDCF => {
            let ac_mid = if instr.d_7_7() != 0 { ctx.dsp.registers.ac1_mid } else { ctx.dsp.registers.ac0_mid };
            let imm = instr.imm_16_31();
            let result = ac_mid & imm;
            let lz = match OP {
                OP_ANDF => result == 0,
                OP_ANDCF => result == imm,
                _ => unreachable!(),
            };
            ctx.dsp.registers.status.set_lz(lz);
        },
        OP_ADDIS => todo!("addis"),
        OP_CMPIS => todo!("cmpis"),
        OP_LRIS => todo!("lris"),
        _ => unreachable!(),
    }
}

#[inline(always)]
pub fn inc_dec<const OP: u32>(
    _ctx: &mut crate::gamecube::GameCube,
    _instr: crate::flipper::dsp::instruction::Instruction,
) {
    match OP {
        OP_INCM => todo!("incm"),
        OP_INC => todo!("inc"),
        OP_DECM => todo!("decm"),
        OP_DEC => todo!("dec"),
        OP_NEG => todo!("neg"),
        OP_ABS_AC => todo!("abs_ac"),
        _ => unreachable!(),
    }
}

#[inline(always)]
pub fn load_store<const OP: u32>(
    ctx: &mut crate::gamecube::GameCube,
    instr: crate::flipper::dsp::instruction::Instruction,
) {
    match OP {
        OP_LRI => {
            ctx.dsp.registers.write::<true>(instr.d_11_15(), instr.imm_16_31());
        },
        OP_LR => todo!("lr"),
        OP_SR => todo!("sr"),
        OP_MRR => todo!("mrr"),
        OP_SI => {
            let addr = 0xFF00 | (instr.mem_8_15_u16());
            ctx.dsp.write_dmem(addr, instr.imm_16_31());
        },
        OP_LRR => todo!("lrr"),
        OP_LRRD => todo!("lrrd"),
        OP_LRRI => todo!("lrri"),
        OP_LRRN => todo!("lrrn"),
        OP_SRR => todo!("srr"),
        OP_SRRD => todo!("srrd"),
        OP_SRRI => todo!("srri"),
        OP_SRRN => todo!("srrn"),
        OP_LRS => {
            let dst = 0x18 + instr.reg_5_7();
            let addr = ((ctx.dsp.registers.config as u16) << 8) | instr.mem_8_15_u16();
            let value = ctx.dsp.read_dmem(addr);
            ctx.dsp.registers.write::<true>(dst, value);
        },
        OP_SRSH | OP_SRS => {
            let addr = ((ctx.dsp.registers.config as u16) << 8) | instr.mem_8_15_u16();
            let src = match OP {
                OP_SRSH => if instr.s_7_7() != 0 { 16 } else { 17 }, // ac0.h (16) or ac1.h (17)
                OP_SRS => 0x1C + instr.reg_6_7(),
                _ => unreachable!(),
            };
            let value = ctx.dsp.registers.read::<true>(src);
            ctx.dsp.write_dmem(addr, value);
        },
        OP_ILRR | OP_ILRRD | OP_ILRRI | OP_ILRRN => {
            let src = instr.s_14_15() as usize;
            let dst = if instr.d_7_7() != 0 { 31u8 } else { 30u8 }; // ac1.m or ac0.m
            let value = ctx.dsp.read_imem(ctx.dsp.registers.ar[src]);
            ctx.dsp.registers.write::<true>(dst, value);
            match OP {
                OP_ILRRD => ctx.dsp.registers.ar[src] = ctx.dsp.registers.ar[src].wrapping_sub(1),
                OP_ILRRI => ctx.dsp.registers.ar[src] = ctx.dsp.registers.ar[src].wrapping_add(1),
                OP_ILRRN => ctx.dsp.registers.ar[src] = ctx.dsp.registers.ar[src].wrapping_add(ctx.dsp.registers.ix[src]),
                _ => {}
            }
        },
        _ => unreachable!(),
    }
}

#[inline(always)]
pub fn logic<const OP: u32>(
    _ctx: &mut crate::gamecube::GameCube,
    _instr: crate::flipper::dsp::instruction::Instruction,
) {
    match OP {
        OP_XORR => todo!("xorr"),
        OP_ANDR => todo!("andr"),
        OP_ORR => todo!("orr"),
        OP_ANDC => todo!("andc"),
        OP_ORC => todo!("orc"),
        OP_XORC => todo!("xorc"),
        OP_NOT_AC => todo!("not_ac"),
        _ => unreachable!(),
    }
}

#[inline(always)]
pub fn loops<const OP: u32>(
    ctx: &mut crate::gamecube::GameCube,
    instr: crate::flipper::dsp::instruction::Instruction,
) {
    match OP {
        OP_LOOP_REG => todo!("loop_reg"),
        OP_BLOOP => {
            let reg = instr.reg_11_15();
            let counter = ctx.dsp.registers.read::<true>(reg);
            let end_addr = instr.addr();

            if counter != 0 {
                ctx.dsp.registers.call_stack.push(ctx.dsp.registers.pc.wrapping_add(2));
                ctx.dsp.registers.loop_addr.push(end_addr);
                ctx.dsp.registers.loop_counter.push(counter);
            } else {
                ctx.dsp.registers.nia = end_addr.wrapping_add(1);
            }
        },
        OP_LOOPI => todo!("loopi"),
        OP_BLOOPI => todo!("bloopi"),
        _ => unreachable!(),
    }
}

#[inline(always)]
pub fn move_ops<const OP: u32>(
    _ctx: &mut crate::gamecube::GameCube,
    _instr: crate::flipper::dsp::instruction::Instruction,
) {
    match OP {
        OP_MOVR => todo!("movr"),
        OP_MOVAX => todo!("movax"),
        OP_MOV => todo!("mov"),
        OP_MOVP => todo!("movp"),
        OP_MOVPZ => todo!("movpz"),
        OP_MOVNP => todo!("movnp"),
        _ => unreachable!(),
    }
}

#[inline(always)]
pub fn mul<const OP: u32>(_ctx: &mut crate::gamecube::GameCube, _instr: crate::flipper::dsp::instruction::Instruction) {
    match OP {
        OP_MUL => todo!("mul"),
        OP_MULX => todo!("mulx"),
        OP_MULC => todo!("mulc"),
        OP_MULAXH => todo!("mulaxh"),
        OP_MULMV => todo!("mulmv"),
        OP_MULMVZ => todo!("mulmvz"),
        OP_MULAC => todo!("mulac"),
        OP_MULXMV => todo!("mulxmv"),
        OP_MULXMVZ => todo!("mulxmvz"),
        OP_MULXAC => todo!("mulxac"),
        OP_MULCMV => todo!("mulcmv"),
        OP_MULCMVZ => todo!("mulcmvz"),
        OP_MULCAC => todo!("mulcac"),
        OP_MADD => todo!("madd"),
        OP_MSUB => todo!("msub"),
        OP_MADDX => todo!("maddx"),
        OP_MSUBX => todo!("msubx"),
        OP_MADDC => todo!("maddc"),
        OP_MSUBC => todo!("msubc"),
        _ => unreachable!(),
    }
}

#[inline(always)]
pub fn shifts<const OP: u32>(
    _ctx: &mut crate::gamecube::GameCube,
    _instr: crate::flipper::dsp::instruction::Instruction,
) {
    match OP {
        OP_LSL => todo!("lsl"),
        OP_LSR => todo!("lsr"),
        OP_ASL => todo!("asl"),
        OP_ASR => todo!("asr"),
        OP_LSL16 => todo!("lsl16"),
        OP_LSR16 => todo!("lsr16"),
        OP_ASR16 => todo!("asr16"),
        OP_LSRN => todo!("lsrn"),
        OP_ASRN => todo!("asrn"),
        OP_LSRNRX => todo!("lsrnrx"),
        OP_ASRNRX => todo!("asrnrx"),
        OP_LSRNR => todo!("lsrnr"),
        OP_ASRNR => todo!("asrnr"),
        _ => unreachable!(),
    }
}

#[inline(always)]
pub fn status<const OP: u32>(
    ctx: &mut crate::gamecube::GameCube,
    instr: crate::flipper::dsp::instruction::Instruction,
) {
    match OP {
        OP_SBCLR => {
            ctx.dsp.registers.status &= !(1 << (6 + instr.bit())) as u16;
        }
        OP_SBSET => {
            ctx.dsp.registers.status |= (1 << (6 + instr.bit())) as u16;
        },
        OP_M2 => {
            ctx.dsp.registers.status.set_am(false);
        },
        OP_M0 => {
            ctx.dsp.registers.status.set_am(true);
        },
        OP_CLR15 => {
            ctx.dsp.registers.status.set_su(false);
        },
        OP_SET15 => {
            ctx.dsp.registers.status.set_su(true);
        },
        OP_SET16 => {
            ctx.dsp.registers.status.set_sxm(SignExtensionMode::Bits16);
        }
        OP_SET40 => {
            ctx.dsp.registers.status.set_sxm(SignExtensionMode::Bits40);
        },
        _ => unreachable!(),
    }
}
