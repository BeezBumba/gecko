use crate::flipper::dsp::lut::*;

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

pub fn control<const OP: u32>(
    _ctx: &mut crate::gamecube::GameCube,
    _instr: crate::flipper::dsp::instruction::Instruction,
) {
    match OP {
        OP_NOP => todo!("nop"),
        OP_HALT => todo!("halt"),
        OP_IFCC => todo!("ifcc"),
        OP_JCC => todo!("jcc"),
        OP_CALLCC => todo!("callcc"),
        OP_RETCC => todo!("retcc"),
        OP_RTICC => todo!("rticc"),
        OP_JRCC => todo!("jrcc"),
        OP_CALLRCC => todo!("callrcc"),
        _ => unreachable!(),
    }
}

pub fn imm_alu<const OP: u32>(
    _ctx: &mut crate::gamecube::GameCube,
    _instr: crate::flipper::dsp::instruction::Instruction,
) {
    match OP {
        OP_ADDI => todo!("addi"),
        OP_XORI => todo!("xori"),
        OP_ANDI => todo!("andi"),
        OP_ORI => todo!("ori"),
        OP_CMPI => todo!("cmpi"),
        OP_ANDF => todo!("andf"),
        OP_ANDCF => todo!("andcf"),
        OP_ADDIS => todo!("addis"),
        OP_CMPIS => todo!("cmpis"),
        OP_LRIS => todo!("lris"),
        _ => unreachable!(),
    }
}

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

pub fn load_store<const OP: u32>(
    _ctx: &mut crate::gamecube::GameCube,
    _instr: crate::flipper::dsp::instruction::Instruction,
) {
    match OP {
        OP_LRI => todo!("lri"),
        OP_LR => todo!("lr"),
        OP_SR => todo!("sr"),
        OP_MRR => todo!("mrr"),
        OP_SI => todo!("si"),
        OP_LRR => todo!("lrr"),
        OP_LRRD => todo!("lrrd"),
        OP_LRRI => todo!("lrri"),
        OP_LRRN => todo!("lrrn"),
        OP_SRR => todo!("srr"),
        OP_SRRD => todo!("srrd"),
        OP_SRRI => todo!("srri"),
        OP_SRRN => todo!("srrn"),
        OP_LRS => todo!("lrs"),
        OP_SRSH => todo!("srsh"),
        OP_SRS => todo!("srs"),
        OP_ILRR => todo!("ilrr"),
        OP_ILRRD => todo!("ilrrd"),
        OP_ILRRI => todo!("ilrri"),
        OP_ILRRN => todo!("ilrrn"),
        _ => unreachable!(),
    }
}

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

pub fn loops<const OP: u32>(
    _ctx: &mut crate::gamecube::GameCube,
    _instr: crate::flipper::dsp::instruction::Instruction,
) {
    match OP {
        OP_LOOP_REG => todo!("loop_reg"),
        OP_BLOOP => todo!("bloop"),
        OP_LOOPI => todo!("loopi"),
        OP_BLOOPI => todo!("bloopi"),
        _ => unreachable!(),
    }
}

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

pub fn status<const OP: u32>(
    _ctx: &mut crate::gamecube::GameCube,
    _instr: crate::flipper::dsp::instruction::Instruction,
) {
    match OP {
        OP_SBCLR => todo!("sbclr"),
        OP_SBSET => todo!("sbset"),
        OP_M2 => todo!("m2"),
        OP_M0 => todo!("m0"),
        OP_CLR15 => todo!("clr15"),
        OP_SET15 => todo!("set15"),
        OP_SET16 => todo!("set16"),
        OP_SET40 => todo!("set40"),
        _ => unreachable!(),
    }
}
