pub const MAX_BLOCK_INSTRS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TermKind {
    Halt,
    Jump,
    Call,
    Ret,
    IfCc,
    LoopSetup,
    LengthLimit,
}

#[derive(Debug, Clone)]
pub struct InstrEntry {
    pub pc: u16,
    pub raw: u32,
    pub size: u8,
}

#[derive(Debug, Clone)]
pub struct BlockSpec {
    pub start_pc: u16,
    pub instrs: Vec<InstrEntry>,
    pub terminator: TermKind,
    pub fallthrough_pc: u16,
}

fn read_imem_word(iram: &[u8], irom: &[u8], addr: u16) -> Option<u16> {
    let off = (addr & 0x0FFF) as usize * 2;
    match addr & 0xF000 {
        0x0000 => Some(u16::from_be_bytes([iram[off], iram[off + 1]])),
        0x8000 => Some(u16::from_be_bytes([irom[off], irom[off + 1]])),
        _ => None,
    }
}

fn classify(primary: u16) -> Option<TermKind> {
    let nibble = (primary >> 12) & 0xF;
    match nibble {
        0x0 | 0x1 => {
            if primary == 0x0021 {
                Some(TermKind::Halt)
            } else {
                Some(TermKind::Jump)
            }
        }
        _ => None,
    }
}

fn approx_size(primary: u16) -> u8 {
    use crate::flipper::dsp::instruction::Instruction;
    use crate::flipper::dsp::lut;
    lut::instr_size(Instruction(primary as u32)) as u8
}

pub fn discover(iram: &[u8], irom: &[u8], start_pc: u16) -> BlockSpec {
    let mut instrs = Vec::with_capacity(8);
    let mut pc = start_pc;
    let mut term = TermKind::LengthLimit;

    while instrs.len() < MAX_BLOCK_INSTRS {
        let Some(w0) = read_imem_word(iram, irom, pc) else {
            term = TermKind::LengthLimit;
            break;
        };

        let size = approx_size(w0);
        let raw = if size == 2 {
            let w1 = read_imem_word(iram, irom, pc.wrapping_add(1)).unwrap_or(0);
            (w0 as u32) | ((w1 as u32) << 16)
        } else {
            w0 as u32
        };
        instrs.push(InstrEntry { pc, raw, size });

        if let Some(kind) = classify(w0) {
            term = kind;
            pc = pc.wrapping_add(size as u16);
            break;
        }

        pc = pc.wrapping_add(size as u16);
    }

    BlockSpec {
        start_pc,
        instrs,
        terminator: term,
        fallthrough_pc: pc,
    }
}
