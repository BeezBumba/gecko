use crate::gekko::instruction::Instruction;
use crate::gekko::condition::ConditionField;
use crate::mmio::{CODE_LINE_BYTES, CODE_LINE_MASK, virt_to_phys};
use crate::system::{System, SystemId};
use rustc_hash::FxHashMap;

const MAX_BLOCK_INSTRS: usize = 256;
#[cfg(target_arch = "wasm32")]
const MAX_CHAIN_BLOCKS: usize = 64;
#[cfg(not(target_arch = "wasm32"))]
const MAX_CHAIN_BLOCKS: usize = 16;
const ENABLE_FAST_PSQ_XFORM: bool = true;
const ENABLE_FAST_PSQ_DFORM: bool = true;
const ENABLE_FAST_OP19_BRANCH_TO_REG: bool = true;
const ENABLE_FAST_OP31_MEMORY_XFORM: bool = true;
const ENABLE_FAST_OP31_DIV_SHIFT_XFORM: bool = true;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TermKind {
    Branch,
    BranchCond,
    BranchToReg,
    SystemCall,
    Rfi,
    Mtmsr,
    Mtspr,
    LengthCap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IdleClass {
    None,
    BranchToSelf,
    PollingLoop,
}

#[derive(Debug, Clone)]
struct DecodedBlock {
    instrs: Vec<u32>,
    pcs: Vec<u32>,
    idle_class: IdleClass,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct BlockStats {
    pub instrs: u32,
    pub blocks: u32,
    pub cache_hits: u32,
    pub cache_misses: u32,
}

pub struct JiterpEngine<const SYSTEM: SystemId> {
    cache: FxHashMap<u32, DecodedBlock>,
    blocks_by_line: FxHashMap<u32, Vec<u32>>,
}

impl<const SYSTEM: SystemId> Default for JiterpEngine<SYSTEM> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const SYSTEM: SystemId> JiterpEngine<SYSTEM> {
    pub fn new() -> Self {
        Self {
            cache: FxHashMap::default(),
            blocks_by_line: FxHashMap::default(),
        }
    }

    pub fn clear_cache(&mut self, mmio: &mut crate::mmio::Mmio<SYSTEM>) {
        let keys: Vec<u32> = self.cache.keys().copied().collect();
        for pc in keys {
            self.unregister_block(mmio, pc);
        }
    }

    pub fn run_block(&mut self, sys: &mut System<SYSTEM>) -> BlockStats {
        let mut stats = BlockStats::default();
        let mut pc = sys.gekko.pc;

        for _ in 0..MAX_CHAIN_BLOCKS {
            let block = if let Some(block) = self.cache.get(&pc) {
                stats.cache_hits = stats.cache_hits.saturating_add(1);
                block
            } else {
                let block = self.discover(sys, pc);
                self.cache.insert(pc, block);
                self.register_block(&mut sys.mmio, pc);
                stats.cache_misses = stats.cache_misses.saturating_add(1);
                // We just inserted this key.
                self.cache.get(&pc).expect("jiterp cache entry missing after insert")
            };

            let mut executed_in_block: u32 = 0;
            for i in 0..block.instrs.len() {
                let cia = block.pcs[i];
                let instr = block.instrs[i];

                #[cfg(target_arch = "wasm32")]
                sys.exec_jiterp_instr_raw_wasm(cia, instr);
                #[cfg(not(target_arch = "wasm32"))]
                sys.exec_decoded_instr_raw(cia, instr);
                executed_in_block = executed_in_block.saturating_add(1);

                if i + 1 < block.pcs.len() {
                    let expected_next = block.pcs[i + 1];
                    if sys.gekko.pc != expected_next {
                        break;
                    }
                }
            }

            if executed_in_block == 0 {
                break;
            }

            stats.blocks = stats.blocks.saturating_add(1);
            stats.instrs = stats.instrs.saturating_add(executed_in_block);

            let next_pc = sys.gekko.pc;

            #[cfg(target_arch = "wasm32")]
            if next_pc == pc
                && block.idle_class != IdleClass::None
                && sys.scheduler.cycles < sys.scheduler.next_deadline()
            {
                sys.scheduler.cycles = sys.scheduler.next_deadline();
                break;
            }

            if next_pc == pc {
                break;
            }
            pc = next_pc;
        }

        stats
    }

    fn discover(&self, sys: &System<SYSTEM>, start_pc: u32) -> DecodedBlock {
        const EXTENSION_MAX_FORWARD_BYTES: u32 = 1024;

        let mut instrs = Vec::with_capacity(8);
        let mut pcs = Vec::with_capacity(8);
        let mut terminator = TermKind::LengthCap;
        let mut pc = start_pc;

        while instrs.len() < MAX_BLOCK_INSTRS {
            let instr = Instruction(sys.mmio.fetch_instruction(pc));
            let cur_pc = pc;

            if let Some(target) = extension_target(instr, cur_pc) {
                if target > cur_pc
                    && target.wrapping_sub(cur_pc) <= EXTENSION_MAX_FORWARD_BYTES
                    && !pcs.contains(&target)
                {
                    pc = target;
                    continue;
                }
            }

            instrs.push(instr.0);
            pcs.push(cur_pc);

            if let Some(t) = classify_terminator(instr) {
                terminator = t;
                break;
            }

            pc = pc.wrapping_add(4);
        }

        let idle_class = classify_idle_loop(&instrs, terminator, start_pc);

        DecodedBlock {
            instrs,
            pcs,
            idle_class,
        }
    }

    fn register_block(&mut self, mmio: &mut crate::mmio::Mmio<SYSTEM>, pc: u32) {
        let Some(spec) = self.cache.get(&pc) else {
            return;
        };

        let mut last = u32::MAX;
        for &vpc in &spec.pcs {
            let line = virt_to_phys(vpc) & CODE_LINE_MASK;
            if line == last {
                continue;
            }
            last = line;
            self.blocks_by_line.entry(line).or_default().push(pc);
            mmio.mark_code(line, CODE_LINE_BYTES);
        }
    }

    fn unregister_block(&mut self, mmio: &mut crate::mmio::Mmio<SYSTEM>, pc: u32) {
        let Some(spec) = self.cache.remove(&pc) else {
            return;
        };

        let mut last = u32::MAX;
        for &vpc in &spec.pcs {
            let line = virt_to_phys(vpc) & CODE_LINE_MASK;
            if line == last {
                continue;
            }
            last = line;

            mmio.unmark_code(line, CODE_LINE_BYTES);
            if let Some(v) = self.blocks_by_line.get_mut(&line) {
                v.retain(|p| *p != pc);
                if v.is_empty() {
                    self.blocks_by_line.remove(&line);
                }
            }
        }
    }
}

#[inline]
fn extension_target(instr: Instruction, pc: u32) -> Option<u32> {
    let word = instr.0;
    let primary = (word >> 26) & 0x3F;
    if primary != 18 {
        return None;
    }

    let lk = (word & 1) != 0;
    if lk {
        return None;
    }

    let aa = ((word >> 1) & 1) != 0;
    let li26 = (word & 0x03FF_FFFC) as i32;
    let li = (li26 << 6) >> 6;

    let target = if aa {
        li as u32
    } else {
        pc.wrapping_add_signed(li)
    };

    Some(target)
}

#[inline]
fn mtspr_is_block_safe(spr: u16) -> bool {
    matches!(
        spr,
        1
            | 8
            | 9
            | 22
            | 26
            | 27
            | 272..=275
            | 912..=919
            | 920
            | 1008
            | 1009
    )
}

#[inline]
fn classify_idle_loop(instrs: &[u32], terminator: TermKind, start_pc: u32) -> IdleClass {
    if let Some(class) = classify_branch_to_self(instrs, terminator, start_pc) {
        return class;
    }

    if classify_polling_loop(instrs, terminator, start_pc) {
        return IdleClass::PollingLoop;
    }

    IdleClass::None
}

#[inline]
fn classify_branch_to_self(instrs: &[u32], terminator: TermKind, start_pc: u32) -> Option<IdleClass> {
    if terminator != TermKind::Branch || instrs.len() != 1 {
        return None;
    }

    let instr = Instruction(instrs[0]);
    if instr.lk() {
        return None;
    }

    let target = if instr.aa() {
        instr.li() as u32
    } else {
        start_pc.wrapping_add_signed(instr.li())
    };

    if target == start_pc {
        Some(IdleClass::BranchToSelf)
    } else {
        None
    }
}

fn classify_polling_loop(instrs: &[u32], terminator: TermKind, start_pc: u32) -> bool {
    const MAX_IDLE_BODY: usize = 6;

    if terminator != TermKind::BranchCond {
        return false;
    }

    let last_idx = match instrs.len().checked_sub(1) {
        Some(i) if i > 0 => i,
        _ => return false,
    };

    if last_idx > MAX_IDLE_BODY {
        return false;
    }

    let term_pc = start_pc.wrapping_add((last_idx as u32) * 4);
    if !is_idle_loop_terminator(Instruction(instrs[last_idx]), term_pc, start_pc) {
        return false;
    }

    validate_idle_loop(&instrs[..last_idx])
}

fn is_idle_loop_terminator(instr: Instruction, branch_pc: u32, block_start_pc: u32) -> bool {
    if primary_opcode(instr) != 16 {
        return false;
    }

    if instr.lk() {
        return false;
    }

    if instr.bo() & 0b00100 == 0 {
        return false;
    }

    let target = if instr.aa() {
        instr.bd() as u32
    } else {
        branch_pc.wrapping_add_signed(instr.bd())
    };

    target == block_start_pc
}

fn validate_idle_loop(body: &[u32]) -> bool {
    let mut write_disallowed: u32 = 0;
    let mut written: u32 = 0;

    for &raw in body {
        let (reads, writes) = match gpr_dataflow(Instruction(raw)) {
            Some(pair) => pair,
            None => return false,
        };

        let externals = reads & !written;
        write_disallowed |= externals;
        if writes & write_disallowed != 0 {
            return false;
        }

        written |= writes;
    }

    true
}

fn gpr_dataflow(instr: Instruction) -> Option<(u32, u32)> {
    let rd_or_s = instr.rd() as u32;
    let ra = instr.ra() as u32;
    let rb = instr.rb() as u32;
    let bit = |r: u32| 1u32 << r;
    let read_a_or_zero = if ra == 0 { 0 } else { bit(ra) };

    Some(match primary_opcode(instr) {
        14 | 15 => (read_a_or_zero, bit(rd_or_s)),
        7 | 8 | 12 | 13 => (bit(ra), bit(rd_or_s)),
        10 | 11 => (bit(ra), 0),
        24 | 25 | 26 | 27 | 28 | 29 => (bit(rd_or_s), bit(ra)),
        20 => (bit(rd_or_s) | bit(ra), bit(ra)),
        21 => (bit(rd_or_s), bit(ra)),
        23 => (bit(rd_or_s) | bit(rb), bit(ra)),
        32 | 34 | 40 | 42 => (read_a_or_zero, bit(rd_or_s)),
        33 | 35 | 41 | 43 => (bit(ra), bit(rd_or_s) | bit(ra)),
        31 => return xform_dataflow(instr),
        _ => return None,
    })
}

fn xform_dataflow(instr: Instruction) -> Option<(u32, u32)> {
    let rd_or_s = instr.rd() as u32;
    let ra = instr.ra() as u32;
    let rb = instr.rb() as u32;
    let bit = |r: u32| 1u32 << r;
    let read_a_or_zero = if ra == 0 { 0 } else { bit(ra) };

    Some(match xo10(instr) {
        266 | 40 | 10 | 138 | 202 | 234 | 8 | 136 | 200 | 232 | 104 | 235 | 75 | 11 | 491 | 459 | 778 | 552 | 522
        | 650 | 714 | 746 | 520 | 648 | 712 | 744 | 616 | 747 | 1003 | 971 => (bit(ra) | bit(rb), bit(rd_or_s)),
        28 | 60 | 124 | 284 | 316 | 412 | 444 | 476 => (bit(rd_or_s) | bit(rb), bit(ra)),
        26 | 922 | 954 => (bit(rd_or_s), bit(ra)),
        24 | 536 | 792 => (bit(rd_or_s) | bit(rb), bit(ra)),
        824 => (bit(rd_or_s), bit(ra)),
        0 | 32 => (bit(ra) | bit(rb), 0),
        19 | 83 | 87 | 279 | 371 | 595 | 659 => (bit(rd_or_s) | read_a_or_zero, bit(rd_or_s)),
        54 | 86 | 246 | 278 | 470 | 758 | 982 | 1014 => (0, 0),
        _ => return None,
    })
}

fn mask(mb: u32, me: u32) -> u32 {
    let begin = 0xFFFF_FFFFu32 >> mb;
    let end = if me >= 31 { 0 } else { 0xFFFF_FFFFu32 >> (me + 1) };
    if mb <= me {
        begin & !end
    } else {
        begin | !end
    }
}

#[inline(always)]
fn primary_opcode(instr: Instruction) -> u8 {
    ((instr.0 >> 26) & 0x3F) as u8
}

#[inline(always)]
fn xo10(instr: Instruction) -> u32 {
    (instr.0 >> 1) & 0x3FF
}

#[inline(always)]
fn add_overflow(a: u32, b: u32, result: u32) -> bool {
    (((a ^ result) & (b ^ result)) >> 31) != 0
}

#[inline(always)]
fn set_overflow<const SYSTEM: SystemId>(ctx: &mut System<SYSTEM>, overflow: bool) {
    ctx.gekko.spr.xer = ctx
        .gekko
        .spr
        .xer
        .with_overflow(overflow)
        .with_summary_overflow(ctx.gekko.spr.xer.summary_overflow() || overflow);
}
#[inline]
fn classify_terminator(instr: Instruction) -> Option<TermKind> {
    let word = instr.0;
    let primary = (word >> 26) & 0x3F;
    match primary {
        16 => Some(TermKind::BranchCond),
        17 => Some(TermKind::SystemCall),
        18 => Some(TermKind::Branch),
        19 => match (word >> 1) & 0x3FF {
            16 | 528 => Some(TermKind::BranchToReg),
            50 => Some(TermKind::Rfi),
            _ => None,
        },
        31 => match (word >> 1) & 0x3FF {
            146 => Some(TermKind::Mtmsr),
            467 => {
                let spr_raw = (word >> 11) & 0x3FF;
                let spr_num = ((spr_raw >> 5) | ((spr_raw & 0x1F) << 5)) as u16;
                if mtspr_is_block_safe(spr_num) {
                    None
                } else {
                    Some(TermKind::Mtspr)
                }
            }
            _ => None,
        },
        _ => None,
    }
}

#[inline(always)]
fn bits_u5(word: u32, shift: u32) -> u8 {
    ((word >> shift) & 0x1F) as u8
}

#[inline(always)]
fn low_u16(word: u32) -> u16 {
    (word & 0xFFFF) as u16
}

#[inline(always)]
fn low_s16(word: u32) -> i32 {
    (low_u16(word) as i16) as i32
}

#[inline(always)]
fn branch_disp_bd(word: u32) -> i32 {
    // BC-form displacement: BD field already shifted by 2 (bits [2..15]).
    let bd = (word & 0x0000_FFFC) as i32;
    (bd << 16) >> 16
}

#[inline(always)]
fn branch_disp_li(word: u32) -> i32 {
    // B-form displacement: LI field already shifted by 2 (bits [2..25]).
    let li = (word & 0x03FF_FFFC) as i32;
    (li << 6) >> 6
}

#[inline(always)]
fn eval_bo<const SYSTEM: SystemId>(ctx: &mut System<SYSTEM>, bo: u8, bi: u8) -> bool {
    let cond = ctx.gekko.cr.get_bit(bi);
    match bo & 0b11110 {
        0b00000 => {
            let ctr = ctx.gekko.spr.ctr.wrapping_sub(1);
            ctx.gekko.spr.ctr = ctr;
            ctr != 0 && !cond
        }
        0b00010 => {
            let ctr = ctx.gekko.spr.ctr.wrapping_sub(1);
            ctx.gekko.spr.ctr = ctr;
            ctr == 0 && !cond
        }
        0b00100 | 0b00110 => !cond,
        0b01000 => {
            let ctr = ctx.gekko.spr.ctr.wrapping_sub(1);
            ctx.gekko.spr.ctr = ctr;
            ctr != 0 && cond
        }
        0b01010 => {
            let ctr = ctx.gekko.spr.ctr.wrapping_sub(1);
            ctx.gekko.spr.ctr = ctr;
            ctr == 0 && cond
        }
        0b01100 | 0b01110 => cond,
        0b10000 | 0b11000 => {
            let ctr = ctx.gekko.spr.ctr.wrapping_sub(1);
            ctx.gekko.spr.ctr = ctr;
            ctr != 0
        }
        0b10010 | 0b11010 => {
            let ctr = ctx.gekko.spr.ctr.wrapping_sub(1);
            ctx.gekko.spr.ctr = ctr;
            ctr == 0
        }
        _ => true,
    }
}

#[inline(always)]
fn ea_disp<const SYSTEM: SystemId>(ctx: &System<SYSTEM>, ra: u8, disp: i32) -> u32 {
    ctx.gekko.read_gpr_or_zero(ra).wrapping_add_signed(disp)
}

#[inline(always)]
fn ea_index<const SYSTEM: SystemId>(ctx: &System<SYSTEM>, ra: u8, rb: u8) -> u32 {
    ctx.gekko
        .read_gpr_or_zero(ra)
        .wrapping_add(ctx.gekko.read_gpr(rb))
}

#[inline(always)]
fn fp_write<const SYSTEM: SystemId>(ctx: &mut System<SYSTEM>, rd: u8, val: f64, rc: bool) {
    ctx.gekko.write_fpr(rd, val);
    if rc {
        ctx.gekko.update_cr1();
    }
}

#[inline(always)]
fn fp_write_single<const SYSTEM: SystemId>(ctx: &mut System<SYSTEM>, rd: u8, val: f64, rc: bool) {
    ctx.gekko.write_fpr(rd, val);
    ctx.gekko.write_ps1(rd, val);
    if rc {
        ctx.gekko.update_cr1();
    }
}

#[inline(always)]
fn fp_compare_f64(a: f64, b: f64) -> ConditionField {
    if a.is_nan() || b.is_nan() {
        ConditionField::new().with_so(true)
    } else if a < b {
        ConditionField::new().with_lt(true)
    } else if a > b {
        ConditionField::new().with_gt(true)
    } else {
        ConditionField::new().with_eq(true)
    }
}

#[inline(always)]
fn round_to_single(val: f64) -> f64 {
    (val as f32) as f64
}

#[inline(always)]
fn ps_write<const SYSTEM: SystemId>(ctx: &mut System<SYSTEM>, rd: u8, ps0: f64, ps1: f64, rc: bool) {
    ctx.gekko.write_fpr(rd, ps0);
    ctx.gekko.write_ps1(rd, ps1);
    if rc {
        ctx.gekko.update_cr1();
    }
}

const DEQUANT_TABLE: [f32; 64] = {
    let mut table = [0.0f32; 64];
    let mut i = 0u32;
    while i < 32 {
        table[i as usize] = 1.0 / (1u64 << i) as f32;
        i += 1;
    }
    while i < 64 {
        table[i as usize] = (1u64 << (64 - i)) as f32;
        i += 1;
    }
    table
};

const QUANT_TABLE: [f32; 64] = {
    let mut table = [0.0f32; 64];
    let mut i = 0u32;
    while i < 32 {
        table[i as usize] = (1u64 << i) as f32;
        i += 1;
    }
    while i < 64 {
        table[i as usize] = 1.0 / (1u64 << (64 - i)) as f32;
        i += 1;
    }
    table
};

#[inline(always)]
fn psq_dequant<const SYSTEM: SystemId>(ctx: &mut System<SYSTEM>, addr: u32, ld_type: u8, ld_scale: u8) -> f64 {
    let scale = DEQUANT_TABLE[ld_scale as usize];
    match ld_type {
        0 => ctx.read_f32(addr),
        4 => (ctx.read_u8_interp(addr) as f32 * scale) as f64,
        5 => (ctx.read_u16_interp(addr) as f32 * scale) as f64,
        6 => (ctx.read_u8_interp(addr) as i8 as f32 * scale) as f64,
        7 => (ctx.read_u16_interp(addr) as i16 as f32 * scale) as f64,
        _ => 0.0,
    }
}

#[inline(always)]
fn psq_quant<const SYSTEM: SystemId>(ctx: &mut System<SYSTEM>, addr: u32, value: f64, st_type: u8, st_scale: u8) {
    let scale = QUANT_TABLE[st_scale as usize];
    match st_type {
        0 => ctx.write_f32(addr, value),
        4 => {
            let v = (value as f32 * scale).clamp(u8::MIN as f32, u8::MAX as f32) as u8;
            ctx.write_u8_interp(addr, v);
        }
        5 => {
            let v = (value as f32 * scale).clamp(u16::MIN as f32, u16::MAX as f32) as u16;
            ctx.write_u16_interp(addr, v);
        }
        6 => {
            let v = (value as f32 * scale).clamp(i8::MIN as f32, i8::MAX as f32) as i8;
            ctx.write_u8_interp(addr, v as u8);
        }
        7 => {
            let v = (value as f32 * scale).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
            ctx.write_u16_interp(addr, v as u16);
        }
        _ => {}
    }
}

#[inline(always)]
fn psq_load<const SYSTEM: SystemId>(ctx: &mut System<SYSTEM>, fd: u8, addr: u32, w: bool, gqr: u32) {
    let ld_type = ((gqr >> 16) & 0x7) as u8;
    let ld_scale = ((gqr >> 24) & 0x3f) as u8;
    let ps0 = psq_dequant(ctx, addr, ld_type, ld_scale);
    let ps1 = if w {
        1.0
    } else {
        let elem_size = match ld_type {
            0 => 4,
            4 | 6 => 1,
            5 | 7 => 2,
            _ => 4,
        };
        psq_dequant(ctx, addr.wrapping_add(elem_size), ld_type, ld_scale)
    };
    ctx.gekko.write_fpr(fd, ps0);
    ctx.gekko.write_ps1(fd, ps1);
}

#[inline(always)]
fn psq_store<const SYSTEM: SystemId>(ctx: &mut System<SYSTEM>, fs: u8, addr: u32, w: bool, gqr: u32) {
    let st_type = (gqr & 0x7) as u8;
    let st_scale = ((gqr >> 8) & 0x3f) as u8;
    let ps0 = ctx.gekko.read_fpr(fs);
    psq_quant(ctx, addr, ps0, st_type, st_scale);
    if !w {
        let ps1 = ctx.gekko.read_ps1(fs);
        let elem_size = match st_type {
            0 => 4,
            4 | 6 => 1,
            5 | 7 => 2,
            _ => 4,
        };
        psq_quant(ctx, addr.wrapping_add(elem_size), ps1, st_type, st_scale);
    }
}

#[inline(always)]
pub(crate) fn try_execute_fast_instruction<const SYSTEM: SystemId>(ctx: &mut System<SYSTEM>, word: u32) -> bool {
    let instr = Instruction(word);
    let op = word >> 26;
    let rd = bits_u5(word, 21);
    let rs = rd;
    let ra = bits_u5(word, 16);

    match op {
        // scalar single-precision FP xforms
        59 => {
            if !ctx.check_fp_available() {
                return true;
            }

            let subop = (word >> 1) & 0xF;
            let rc = (word & 1) != 0;
            let handled = match subop {
                2 => {
                    let a = ctx.gekko.read_fpr(ra);
                    let b = ctx.gekko.read_fpr(bits_u5(word, 11));
                    fp_write_single(ctx, rd, (a / b) as f32 as f64, rc);
                    true
                }
                4 => {
                    let a = ctx.gekko.read_fpr(ra);
                    let b = ctx.gekko.read_fpr(bits_u5(word, 11));
                    fp_write_single(ctx, rd, (a - b) as f32 as f64, rc);
                    true
                }
                5 => {
                    let a = ctx.gekko.read_fpr(ra);
                    let b = ctx.gekko.read_fpr(bits_u5(word, 11));
                    fp_write_single(ctx, rd, (a + b) as f32 as f64, rc);
                    true
                }
                6 => {
                    let b = ctx.gekko.read_fpr(bits_u5(word, 11));
                    fp_write_single(ctx, rd, b.sqrt() as f32 as f64, rc);
                    true
                }
                8 => {
                    let b = ctx.gekko.read_fpr(bits_u5(word, 11)) as f32;
                    fp_write_single(ctx, rd, (1.0f32 / b) as f64, rc);
                    true
                }
                9 => {
                    let a = ctx.gekko.read_fpr(ra);
                    let c = ctx.gekko.read_fpr(bits_u5(word, 6));
                    fp_write_single(ctx, rd, (a * c) as f32 as f64, rc);
                    true
                }
                12..=15 => {
                    let a = ctx.gekko.read_fpr(ra);
                    let c = ctx.gekko.read_fpr(bits_u5(word, 6));
                    let b = ctx.gekko.read_fpr(bits_u5(word, 11));
                    let val = match subop {
                        12 => a * c - b,
                        13 => a * c + b,
                        14 => -(a * c - b),
                        15 => -(a * c + b),
                        _ => unreachable!(),
                    };
                    fp_write_single(ctx, rd, val as f32 as f64, rc);
                    true
                }
                _ => false,
            };

            if handled {
                ctx.check_fp_program_exception();
            }
            return handled;
        }
        // scalar FP xforms
        63 => {
            if !ctx.check_fp_available() {
                return true;
            }

            let xo10 = (word >> 1) & 0x3FF;
            let rc = (word & 1) != 0;
            let handled = match xo10 {
                // fcmpu / fcmpo
                0 | 32 => {
                    let crfd = bits_u5(word, 23) & 0x7;
                    let a = ctx.gekko.read_fpr(ra);
                    let b = ctx.gekko.read_fpr(bits_u5(word, 11));
                    ctx.gekko.cr.set_field(crfd, fp_compare_f64(a, b));
                    true
                }
                // fmr
                72 => {
                    fp_write(ctx, rd, ctx.gekko.read_fpr(bits_u5(word, 11)), rc);
                    true
                }
                // fnabs
                136 => {
                    let v = ctx.gekko.read_fpr(bits_u5(word, 11));
                    fp_write(ctx, rd, -v.abs(), rc);
                    true
                }
                // fneg
                40 => {
                    let v = ctx.gekko.read_fpr(bits_u5(word, 11));
                    fp_write(ctx, rd, -v, rc);
                    true
                }
                // fabs
                264 => {
                    let v = ctx.gekko.read_fpr(bits_u5(word, 11));
                    fp_write(ctx, rd, v.abs(), rc);
                    true
                }
                // frsp
                12 => {
                    let v = ctx.gekko.read_fpr(bits_u5(word, 11)) as f32 as f64;
                    fp_write_single(ctx, rd, v, rc);
                    true
                }
                // fctiw / fctiwz
                14 | 15 => {
                    let v = ctx.gekko.read_fpr(bits_u5(word, 11));
                    let res = if xo10 == 14 { v.round() as i32 } else { v as i32 };
                    ctx.gekko.write_fpr(rd, f64::from_bits(res as u32 as u64));
                    if rc {
                        ctx.gekko.update_cr1();
                    }
                    true
                }
                // fdiv
                18 | 583 => {
                    let a = ctx.gekko.read_fpr(ra);
                    let b = ctx.gekko.read_fpr(bits_u5(word, 11));
                    fp_write(ctx, rd, a / b, rc);
                    true
                }
                // fsub
                20 => {
                    let a = ctx.gekko.read_fpr(ra);
                    let b = ctx.gekko.read_fpr(bits_u5(word, 11));
                    fp_write(ctx, rd, a - b, rc);
                    true
                }
                // fadd
                21 => {
                    let a = ctx.gekko.read_fpr(ra);
                    let b = ctx.gekko.read_fpr(bits_u5(word, 11));
                    fp_write(ctx, rd, a + b, rc);
                    true
                }
                // fsqrt
                22 | 711 => {
                    let b = ctx.gekko.read_fpr(bits_u5(word, 11));
                    fp_write(ctx, rd, b.sqrt(), rc);
                    true
                }
                // fsel
                23 => {
                    let a = ctx.gekko.read_fpr(ra);
                    let c = ctx.gekko.read_fpr(bits_u5(word, 6));
                    let b = ctx.gekko.read_fpr(bits_u5(word, 11));
                    fp_write(ctx, rd, if a >= 0.0 { c } else { b }, rc);
                    true
                }
                // fres
                24 => {
                    let b = ctx.gekko.read_fpr(bits_u5(word, 11)) as f32;
                    fp_write_single(ctx, rd, (1.0f32 / b) as f64, rc);
                    true
                }
                // fmul
                25 => {
                    let a = ctx.gekko.read_fpr(ra);
                    let c = ctx.gekko.read_fpr(bits_u5(word, 6));
                    fp_write(ctx, rd, a * c, rc);
                    true
                }
                // frsqrte
                26 => {
                    let b = ctx.gekko.read_fpr(bits_u5(word, 11));
                    fp_write(ctx, rd, 1.0 / b.sqrt(), rc);
                    true
                }
                // fmadd / fmsub / fnmadd / fnmsub
                28 | 29 | 30 | 31 => {
                    let a = ctx.gekko.read_fpr(ra);
                    let c = ctx.gekko.read_fpr(bits_u5(word, 6));
                    let b = ctx.gekko.read_fpr(bits_u5(word, 11));
                    let core = if xo10 == 29 || xo10 == 31 {
                        a * c + b
                    } else {
                        a * c - b
                    };
                    let res = if xo10 == 30 || xo10 == 31 { -core } else { core };
                    fp_write(ctx, rd, res, rc);
                    true
                }
                _ => false,
            };

            if handled {
                ctx.check_fp_program_exception();
            }
            return handled;
        }
        // paired-single arithmetic and compare xforms
        4 => {
            if !ctx.check_fp_available() {
                return true;
            }

            let subop = (word >> 1) & 0x1F;
            let fc = bits_u5(word, 6);
            let rb_ps = bits_u5(word, 11);
            let ra_ps = bits_u5(word, 16);
            let rc = (word & 1) != 0;

            let handled = match subop {
                0 => {
                    let crfd = ((word >> 23) & 0x7) as u8;
                    let cmp_kind = (word >> 6) & 0x3;
                    match cmp_kind {
                        0 | 1 => {
                            let a = ctx.gekko.read_fpr(ra_ps);
                            let b = ctx.gekko.read_fpr(rb_ps);
                            ctx.gekko.cr.set_field(crfd, fp_compare_f64(a, b));
                            true
                        }
                        2 | 3 => {
                            let a = ctx.gekko.read_ps1(ra_ps);
                            let b = ctx.gekko.read_ps1(rb_ps);
                            ctx.gekko.cr.set_field(crfd, fp_compare_f64(a, b));
                            true
                        }
                        _ => false,
                    }
                }
                6 => {
                    if !ENABLE_FAST_PSQ_XFORM {
                        return false;
                    }
                    let ea = ctx
                        .gekko
                        .read_gpr_or_zero(instr.ra())
                        .wrapping_add(ctx.gekko.read_gpr(instr.rb()));
                    let gqr = ctx.gekko.spr.read_gqr(instr.psq_ix());
                    let w = instr.psq_wx();
                    psq_load(ctx, rd, ea, w, gqr);
                    if ((word >> 6) & 1) != 0 {
                        ctx.gekko.write_gpr(instr.ra(), ea);
                    }
                    true
                }
                7 => {
                    if !ENABLE_FAST_PSQ_XFORM {
                        return false;
                    }
                    let ea = ctx
                        .gekko
                        .read_gpr_or_zero(instr.ra())
                        .wrapping_add(ctx.gekko.read_gpr(instr.rb()));
                    let gqr = ctx.gekko.spr.read_gqr(instr.psq_ix());
                    let w = instr.psq_wx();
                    psq_store(ctx, rd, ea, w, gqr);
                    if ((word >> 6) & 1) != 0 {
                        ctx.gekko.write_gpr(instr.ra(), ea);
                    }
                    true
                }
                8 => {
                    let unary = ((word >> 6) & 0xF) as u32;
                    let b0 = ctx.gekko.read_fpr(rb_ps);
                    let b1 = ctx.gekko.read_ps1(rb_ps);
                    let (p0, p1) = match unary {
                        1 => (-b0, -b1),
                        2 => (b0, b1),
                        4 => (-b0.abs(), -b1.abs()),
                        8 => (b0.abs(), b1.abs()),
                        _ => return false,
                    };
                    ps_write(ctx, rd, p0, p1, rc);
                    true
                }
                10 => {
                    let p0 = round_to_single(ctx.gekko.read_fpr(ra_ps) + ctx.gekko.read_ps1(rb_ps));
                    let p1 = ctx.gekko.read_ps1(fc);
                    ps_write(ctx, rd, p0, p1, rc);
                    true
                }
                11 => {
                    let p0 = ctx.gekko.read_fpr(fc);
                    let p1 = round_to_single(ctx.gekko.read_fpr(ra_ps) + ctx.gekko.read_ps1(rb_ps));
                    ps_write(ctx, rd, p0, p1, rc);
                    true
                }
                12 => {
                    let c0 = ctx.gekko.read_fpr(fc);
                    let p0 = round_to_single(ctx.gekko.read_fpr(ra_ps) * c0);
                    let p1 = round_to_single(ctx.gekko.read_ps1(ra_ps) * c0);
                    ps_write(ctx, rd, p0, p1, rc);
                    true
                }
                13 => {
                    let c1 = ctx.gekko.read_ps1(fc);
                    let p0 = round_to_single(ctx.gekko.read_fpr(ra_ps) * c1);
                    let p1 = round_to_single(ctx.gekko.read_ps1(ra_ps) * c1);
                    ps_write(ctx, rd, p0, p1, rc);
                    true
                }
                14 => {
                    let c0 = ctx.gekko.read_fpr(fc);
                    let p0 = round_to_single(ctx.gekko.read_fpr(ra_ps) * c0 + ctx.gekko.read_fpr(rb_ps));
                    let p1 = round_to_single(ctx.gekko.read_ps1(ra_ps) * c0 + ctx.gekko.read_ps1(rb_ps));
                    ps_write(ctx, rd, p0, p1, rc);
                    true
                }
                15 => {
                    let c1 = ctx.gekko.read_ps1(fc);
                    let p0 = round_to_single(ctx.gekko.read_fpr(ra_ps) * c1 + ctx.gekko.read_fpr(rb_ps));
                    let p1 = round_to_single(ctx.gekko.read_ps1(ra_ps) * c1 + ctx.gekko.read_ps1(rb_ps));
                    ps_write(ctx, rd, p0, p1, rc);
                    true
                }
                16 => {
                    let merge_kind = (word >> 6) & 0x3;
                    let p0 = match merge_kind {
                        0 | 1 => ctx.gekko.read_fpr(ra_ps),
                        2 | 3 => ctx.gekko.read_ps1(ra_ps),
                        _ => return false,
                    };
                    let p1 = match merge_kind {
                        0 | 2 => ctx.gekko.read_fpr(rb_ps),
                        1 | 3 => ctx.gekko.read_ps1(rb_ps),
                        _ => return false,
                    };
                    ps_write(ctx, rd, p0, p1, rc);
                    true
                }
                18 => {
                    let b0 = ctx.gekko.read_fpr(rb_ps);
                    let b1 = ctx.gekko.read_ps1(rb_ps);
                    ps_write(ctx, rd, (ctx.gekko.read_fpr(ra_ps) / b0) as f32 as f64, (ctx.gekko.read_ps1(ra_ps) / b1) as f32 as f64, rc);
                    true
                }
                20 => {
                    let b0 = ctx.gekko.read_fpr(rb_ps);
                    let b1 = ctx.gekko.read_ps1(rb_ps);
                    ps_write(ctx, rd, round_to_single(ctx.gekko.read_fpr(ra_ps) - b0), round_to_single(ctx.gekko.read_ps1(ra_ps) - b1), rc);
                    true
                }
                21 => {
                    let b0 = ctx.gekko.read_fpr(rb_ps);
                    let b1 = ctx.gekko.read_ps1(rb_ps);
                    ps_write(ctx, rd, round_to_single(ctx.gekko.read_fpr(ra_ps) + b0), round_to_single(ctx.gekko.read_ps1(ra_ps) + b1), rc);
                    true
                }
                23 => {
                    let a0 = ctx.gekko.read_fpr(ra_ps);
                    let a1 = ctx.gekko.read_ps1(ra_ps);
                    let c0 = ctx.gekko.read_fpr(fc);
                    let c1 = ctx.gekko.read_ps1(fc);
                    let b0 = ctx.gekko.read_fpr(rb_ps);
                    let b1 = ctx.gekko.read_ps1(rb_ps);
                    let p0 = if a0 >= 0.0 { c0 } else { b0 };
                    let p1 = if a1 >= 0.0 { c1 } else { b1 };
                    ps_write(ctx, rd, p0, p1, rc);
                    true
                }
                24 => {
                    let b0 = ctx.gekko.read_fpr(rb_ps);
                    let b1 = ctx.gekko.read_ps1(rb_ps);
                    ps_write(ctx, rd, (1.0f32 / b0 as f32) as f64, (1.0f32 / b1 as f32) as f64, rc);
                    true
                }
                25 => {
                    let c0 = ctx.gekko.read_fpr(fc);
                    let c1 = ctx.gekko.read_ps1(fc);
                    ps_write(ctx, rd, round_to_single(ctx.gekko.read_fpr(ra_ps) * c0), round_to_single(ctx.gekko.read_ps1(ra_ps) * c1), rc);
                    true
                }
                26 => {
                    let b0 = ctx.gekko.read_fpr(rb_ps);
                    let b1 = ctx.gekko.read_ps1(rb_ps);
                    ps_write(
                        ctx,
                        rd,
                        (1.0f32 / (b0 as f32).sqrt()) as f64,
                        (1.0f32 / (b1 as f32).sqrt()) as f64,
                        rc,
                    );
                    true
                }
                28 => {
                    let b0 = ctx.gekko.read_fpr(rb_ps);
                    let b1 = ctx.gekko.read_ps1(rb_ps);
                    let c0 = ctx.gekko.read_fpr(fc);
                    let c1 = ctx.gekko.read_ps1(fc);
                    ps_write(ctx, rd, round_to_single(ctx.gekko.read_fpr(ra_ps) * c0 - b0), round_to_single(ctx.gekko.read_ps1(ra_ps) * c1 - b1), rc);
                    true
                }
                29 => {
                    let b0 = ctx.gekko.read_fpr(rb_ps);
                    let b1 = ctx.gekko.read_ps1(rb_ps);
                    let c0 = ctx.gekko.read_fpr(fc);
                    let c1 = ctx.gekko.read_ps1(fc);
                    ps_write(ctx, rd, round_to_single(ctx.gekko.read_fpr(ra_ps) * c0 + b0), round_to_single(ctx.gekko.read_ps1(ra_ps) * c1 + b1), rc);
                    true
                }
                30 => {
                    let b0 = ctx.gekko.read_fpr(rb_ps);
                    let b1 = ctx.gekko.read_ps1(rb_ps);
                    let c0 = ctx.gekko.read_fpr(fc);
                    let c1 = ctx.gekko.read_ps1(fc);
                    ps_write(ctx, rd, round_to_single(-(ctx.gekko.read_fpr(ra_ps) * c0 - b0)), round_to_single(-(ctx.gekko.read_ps1(ra_ps) * c1 - b1)), rc);
                    true
                }
                31 => {
                    let b0 = ctx.gekko.read_fpr(rb_ps);
                    let b1 = ctx.gekko.read_ps1(rb_ps);
                    let c0 = ctx.gekko.read_fpr(fc);
                    let c1 = ctx.gekko.read_ps1(fc);
                    ps_write(ctx, rd, round_to_single(-(ctx.gekko.read_fpr(ra_ps) * c0 + b0)), round_to_single(-(ctx.gekko.read_ps1(ra_ps) * c1 + b1)), rc);
                    true
                }
                _ => false,
            };

            if handled {
                ctx.check_fp_program_exception();
            }
            return handled;
        }
        // psq_l / psq_lu / psq_st / psq_stu
        56 | 57 | 60 | 61 => {
            if !ctx.check_fp_available() {
                return true;
            }

            if !ENABLE_FAST_PSQ_DFORM {
                return false;
            }

            let base = ctx.gekko.read_gpr_or_zero(instr.ra());
            let ea = base.wrapping_add_signed(instr.disp_psq());
            let gqr = ctx.gekko.spr.read_gqr(instr.psq_i());
            let w = instr.psq_w();

            if op == 56 || op == 57 {
                psq_load(ctx, instr.fd(), ea, w, gqr);
                if op == 57 {
                    ctx.gekko.write_gpr(instr.ra(), ea);
                }
            } else {
                psq_store(ctx, instr.fs(), ea, w, gqr);
                if op == 61 {
                    ctx.gekko.write_gpr(instr.ra(), ea);
                }
            }

            true
        }
        // lfs / lfsu / lfd / lfdu / stfs / stfsu / stfd / stfdu
        48..=55 => {
            if !ctx.check_fp_available() {
                return true;
            }

            let addr = ea_disp(ctx, instr.ra(), instr.disp());
            match op {
                48 => {
                    let val = ctx.read_f32(addr);
                    ctx.gekko.write_fpr(instr.rd(), val);
                    ctx.gekko.write_ps1(instr.rd(), val);
                }
                49 => {
                    let val = ctx.read_f32(addr);
                    ctx.gekko.write_fpr(instr.rd(), val);
                    ctx.gekko.write_ps1(instr.rd(), val);
                    ctx.gekko.write_gpr(instr.ra(), addr);
                }
                50 => {
                    let val = ctx.read_f64(addr);
                    ctx.gekko.write_fpr(instr.rd(), val);
                }
                51 => {
                    let val = ctx.read_f64(addr);
                    ctx.gekko.write_fpr(instr.rd(), val);
                    ctx.gekko.write_gpr(instr.ra(), addr);
                }
                52 => {
                    ctx.write_f32(addr, ctx.gekko.read_fpr(instr.rs()));
                }
                53 => {
                    ctx.write_f32(addr, ctx.gekko.read_fpr(instr.rs()));
                    ctx.gekko.write_gpr(instr.ra(), addr);
                }
                54 => {
                    ctx.write_f64(addr, ctx.gekko.read_fpr(instr.rs()));
                }
                55 => {
                    ctx.write_f64(addr, ctx.gekko.read_fpr(instr.rs()));
                    ctx.gekko.write_gpr(instr.ra(), addr);
                }
                _ => unreachable!(),
            }

            true
        }
        // mulli
        7 => {
            let lhs = ctx.gekko.read_gpr(ra) as i32 as i64;
            let rhs = low_s16(word) as i64;
            ctx.gekko.write_gpr(rd, lhs.wrapping_mul(rhs) as u32);
            true
        }
        // subfic
        8 => {
            let ra_val = ctx.gekko.read_gpr(ra);
            let simm = low_s16(word) as u32;
            ctx.gekko.write_gpr(rd, simm.wrapping_sub(ra_val));
            ctx.gekko.spr.xer = ctx.gekko.spr.xer.with_carry(simm >= ra_val);
            true
        }
        // addic
        12 => {
            let ra_val = ctx.gekko.read_gpr(ra);
            let (res, carry) = ra_val.overflowing_add(low_s16(word) as u32);
            ctx.gekko.write_gpr(rd, res);
            ctx.gekko.spr.xer = ctx.gekko.spr.xer.with_carry(carry);
            true
        }
        // addic.
        13 => {
            let ra_val = ctx.gekko.read_gpr(ra);
            let (res, carry) = ra_val.overflowing_add(low_s16(word) as u32);
            ctx.gekko.write_gpr(rd, res);
            ctx.gekko.spr.xer = ctx.gekko.spr.xer.with_carry(carry);
            ctx.gekko.update_cr0(res);
            true
        }
        // addi
        14 => {
            let ra_val = ctx.gekko.read_gpr_or_zero(ra);
            let simm = low_s16(word);
            ctx.gekko.write_gpr(rd, ra_val.wrapping_add_signed(simm));
            true
        }
        // rlwimi
        20 => {
            let sh = (word >> 11) & 0x1F;
            let mb = (word >> 6) & 0x1F;
            let me = (word >> 1) & 0x1F;
            let rc = (word & 1) != 0;
            let m = mask(mb, me);
            let r = ctx.gekko.read_gpr(rs).rotate_left(sh);
            let res = (r & m) | (ctx.gekko.read_gpr(ra) & !m);
            ctx.gekko.write_gpr(ra, res);
            if rc {
                ctx.gekko.update_cr0(res);
            }
            true
        }
        // rlwinm
        21 => {
            let sh = (word >> 11) & 0x1F;
            let mb = (word >> 6) & 0x1F;
            let me = (word >> 1) & 0x1F;
            let rc = (word & 1) != 0;
            let m = mask(mb, me);
            let res = ctx.gekko.read_gpr(rs).rotate_left(sh) & m;
            ctx.gekko.write_gpr(ra, res);
            if rc {
                ctx.gekko.update_cr0(res);
            }
            true
        }
        // addis
        15 => {
            let ra_val = ctx.gekko.read_gpr_or_zero(ra);
            let simm = low_s16(word) << 16;
            ctx.gekko.write_gpr(rd, ra_val.wrapping_add_signed(simm));
            true
        }
        // rlwnm
        23 => {
            let rb = bits_u5(word, 11);
            let mb = (word >> 6) & 0x1F;
            let me = (word >> 1) & 0x1F;
            let rc = (word & 1) != 0;
            let m = mask(mb, me);
            let sh = ctx.gekko.read_gpr(rb) & 0x1F;
            let res = ctx.gekko.read_gpr(rs).rotate_left(sh) & m;
            ctx.gekko.write_gpr(ra, res);
            if rc {
                ctx.gekko.update_cr0(res);
            }
            true
        }
        // ori
        24 => {
            let imm = low_u16(word) as u32;
            ctx.gekko.write_gpr(ra, ctx.gekko.read_gpr(rs) | imm);
            true
        }
        // oris
        25 => {
            let imm = (low_u16(word) as u32) << 16;
            ctx.gekko.write_gpr(ra, ctx.gekko.read_gpr(rs) | imm);
            true
        }
        // xori
        26 => {
            let imm = low_u16(word) as u32;
            ctx.gekko.write_gpr(ra, ctx.gekko.read_gpr(rs) ^ imm);
            true
        }
        // xoris
        27 => {
            let imm = (low_u16(word) as u32) << 16;
            ctx.gekko.write_gpr(ra, ctx.gekko.read_gpr(rs) ^ imm);
            true
        }
        // andi.
        28 => {
            let mask = low_u16(word) as u32;
            let val = ctx.gekko.read_gpr(rs) & mask;
            ctx.gekko.write_gpr(ra, val);
            ctx.gekko.update_cr0(val);
            true
        }
        // andis.
        29 => {
            let mask = (low_u16(word) as u32) << 16;
            let val = ctx.gekko.read_gpr(rs) & mask;
            ctx.gekko.write_gpr(ra, val);
            ctx.gekko.update_cr0(val);
            true
        }
        // b
        18 => {
            let aa = ((word >> 1) & 1) != 0;
            let lk = (word & 1) != 0;
            let li = branch_disp_li(word);
            ctx.gekko.nia = if aa {
                li as u32
            } else {
                ctx.gekko.cia.wrapping_add_signed(li)
            };
            if lk {
                ctx.gekko.spr.lr = ctx.gekko.cia.wrapping_add(4);
            }
            true
        }
        // bc
        16 => {
            let bo = bits_u5(word, 21);
            let bi = bits_u5(word, 16);
            if !eval_bo(ctx, bo, bi) {
                return true;
            }

            let aa = ((word >> 1) & 1) != 0;
            let lk = (word & 1) != 0;
            let bd = branch_disp_bd(word);
            ctx.gekko.nia = if aa {
                bd as u32
            } else {
                ctx.gekko.cia.wrapping_add_signed(bd)
            };
            if lk {
                ctx.gekko.spr.lr = ctx.gekko.cia.wrapping_add(4);
            }
            true
        }
        // sc
        17 => {
            ctx.cause_syscall_interrupt();
            true
        }
        // cmpli
        10 => {
            let crfd = bits_u5(word, 23) & 0x7;
            let a = ctx.gekko.read_gpr(ra);
            let b = low_u16(word) as u32;
            let field = ConditionField::new()
                .with_lt(a < b)
                .with_gt(a > b)
                .with_eq(a == b)
                .with_so(ctx.gekko.spr.xer.summary_overflow());
            ctx.gekko.cr.set_field(crfd, field);
            true
        }
        // cmpi
        11 => {
            let crfd = bits_u5(word, 23) & 0x7;
            let a = ctx.gekko.read_gpr(ra) as i32;
            let b = low_s16(word);
            let field = ConditionField::new()
                .with_lt(a < b)
                .with_gt(a > b)
                .with_eq(a == b)
                .with_so(ctx.gekko.spr.xer.summary_overflow());
            ctx.gekko.cr.set_field(crfd, field);
            true
        }
        // op31 variants where we can safely fast-path branch forms:
        // bclr / bcctr, compare, and common indexed load/store forms.
        19 => {
            if !ENABLE_FAST_OP19_BRANCH_TO_REG {
                return false;
            }

            let xo10 = (word >> 1) & 0x3FF;
            if xo10 == 16 || xo10 == 528 {
                let bo = bits_u5(word, 21);
                let bi = bits_u5(word, 16);
                let lk = (word & 1) != 0;

                if xo10 == 16 {
                    if !eval_bo(ctx, bo, bi) {
                        return true;
                    }
                    ctx.gekko.nia = ctx.gekko.spr.lr & !3;
                    if lk {
                        ctx.gekko.spr.lr = ctx.gekko.cia.wrapping_add(4);
                    }
                    return true;
                }

                let condition = (bo & 0x10) != 0 || (ctx.gekko.cr.get_bit(bi) == ((bo & 0x08) != 0));
                if !condition {
                    return true;
                }
                ctx.gekko.nia = ctx.gekko.spr.ctr & !3;
                if lk {
                    ctx.gekko.spr.lr = ctx.gekko.cia.wrapping_add(4);
                }
                return true;
            }

            match xo10 {
                // mcrf
                0 => {
                    let crfd = bits_u5(word, 23) & 0x7;
                    let crfs = bits_u5(word, 18) & 0x7;
                    let src = ctx.gekko.cr.get_field(crfs);
                    ctx.gekko.cr.set_field(crfd, src);
                    true
                }
                // rfi
                50 => {
                    const RFI_MSR_MASK: u32 = 0x87C0_FFFF;
                    let msr = (ctx.gekko.msr.raw() & !RFI_MSR_MASK) | (ctx.gekko.spr.srr1 & RFI_MSR_MASK);
                    ctx.gekko.msr = crate::gekko::msr::Msr::from(msr & !0x0004_0000);
                    ctx.gekko.nia = ctx.gekko.spr.srr0.value() << 2;
                    true
                }
                // isync
                150 => true,
                // crnor / crandc / crxor / crnand / crand / creqv / crorc / cror
                33 | 129 | 193 | 225 | 257 | 289 | 417 | 449 => {
                    let crbd = bits_u5(word, 21);
                    let crba = bits_u5(word, 16);
                    let crbb = bits_u5(word, 11);
                    let a = ctx.gekko.cr.get_bit(crba);
                    let b = ctx.gekko.cr.get_bit(crbb);
                    let result = match xo10 {
                        33 => !(a | b),
                        129 => a & !b,
                        193 => a ^ b,
                        225 => !(a & b),
                        257 => a & b,
                        289 => a == b,
                        417 => a | !b,
                        449 => a | b,
                        _ => unreachable!(),
                    };
                    ctx.gekko.cr.set_bit(crbd, result);
                    true
                }
                _ => false,
            }
        }
        31 => {
            let xo10 = (word >> 1) & 0x3FF;
            let rb = bits_u5(word, 11);

            let is_memory_xform = matches!(xo10, 23 | 55 | 87 | 119 | 151 | 183 | 215 | 247 | 279 | 311 | 343 | 375 | 407 | 439 | 535 | 663);
            if is_memory_xform && !ENABLE_FAST_OP31_MEMORY_XFORM {
                return false;
            }

            let is_div_shift_xform = matches!(xo10, 459 | 491 | 536 | 792 | 824);
            if is_div_shift_xform && !ENABLE_FAST_OP31_DIV_SHIFT_XFORM {
                return false;
            }

            match xo10 {
                // mfcr
                19 => {
                    ctx.gekko.write_gpr(rd, ctx.gekko.cr.raw());
                    true
                }
                // cmp
                0 => {
                    let crfd = bits_u5(word, 23) & 0x7;
                    let a = ctx.gekko.read_gpr(ra) as i32;
                    let b = ctx.gekko.read_gpr(rb) as i32;
                    let field = ConditionField::new()
                        .with_lt(a < b)
                        .with_gt(a > b)
                        .with_eq(a == b)
                        .with_so(ctx.gekko.spr.xer.summary_overflow());
                    ctx.gekko.cr.set_field(crfd, field);
                    true
                }
                // subfc
                8 => {
                    let ra_val = ctx.gekko.read_gpr(ra);
                    let rb_val = ctx.gekko.read_gpr(rb);
                    let res = rb_val.wrapping_sub(ra_val);
                    ctx.gekko.spr.xer = ctx.gekko.spr.xer.with_carry(rb_val >= ra_val);
                    ctx.gekko.write_gpr(rd, res);
                    if (word & (1 << 10)) != 0 {
                        set_overflow(ctx, add_overflow(!ra_val, rb_val, res));
                    }
                    if (word & 1) != 0 {
                        ctx.gekko.update_cr0(res);
                    }
                    true
                }
                // addc
                10 => {
                    let ra_val = ctx.gekko.read_gpr(ra);
                    let rb_val = ctx.gekko.read_gpr(rb);
                    let (res, carry) = ra_val.overflowing_add(rb_val);
                    ctx.gekko.write_gpr(rd, res);
                    ctx.gekko.spr.xer = ctx.gekko.spr.xer.with_carry(carry);
                    if (word & (1 << 10)) != 0 {
                        set_overflow(ctx, add_overflow(ra_val, rb_val, res));
                    }
                    if (word & 1) != 0 {
                        ctx.gekko.update_cr0(res);
                    }
                    true
                }
                // mulhwu
                11 => {
                    let res = ((ctx.gekko.read_gpr(ra) as u64 * ctx.gekko.read_gpr(rb) as u64) >> 32) as u32;
                    ctx.gekko.write_gpr(rd, res);
                    if (word & 1) != 0 {
                        ctx.gekko.update_cr0(res);
                    }
                    true
                }
                // lwzx
                23 => {
                    let addr = ea_index(ctx, ra, rb);
                    let val = ctx.read_u32_interp(addr);
                    ctx.gekko.write_gpr(rd, val);
                    true
                }
                // slw
                24 => {
                    let rs_val = ctx.gekko.read_gpr(rs);
                    let rb_val = ctx.gekko.read_gpr(rb);
                    let sh = rb_val & 0x3F;
                    let res = if sh >= 32 { 0 } else { rs_val << sh };
                    ctx.gekko.write_gpr(ra, res);
                    if (word & 1) != 0 {
                        ctx.gekko.update_cr0(res);
                    }
                    true
                }
                // cntlzw
                26 => {
                    let res = ctx.gekko.read_gpr(rs).leading_zeros();
                    ctx.gekko.write_gpr(ra, res);
                    if (word & 1) != 0 {
                        ctx.gekko.update_cr0(res);
                    }
                    true
                }
                // and
                28 => {
                    let res = ctx.gekko.read_gpr(rs) & ctx.gekko.read_gpr(rb);
                    ctx.gekko.write_gpr(ra, res);
                    if (word & 1) != 0 {
                        ctx.gekko.update_cr0(res);
                    }
                    true
                }
                // cmpl
                32 => {
                    let crfd = bits_u5(word, 23) & 0x7;
                    let a = ctx.gekko.read_gpr(ra);
                    let b = ctx.gekko.read_gpr(rb);
                    let field = ConditionField::new()
                        .with_lt(a < b)
                        .with_gt(a > b)
                        .with_eq(a == b)
                        .with_so(ctx.gekko.spr.xer.summary_overflow());
                    ctx.gekko.cr.set_field(crfd, field);
                    true
                }
                // subf
                40 => {
                    let a = !ctx.gekko.read_gpr(ra);
                    let b = ctx.gekko.read_gpr(rb);
                    let res = a.wrapping_add(b).wrapping_add(1);
                    ctx.gekko.write_gpr(rd, res);
                    if (word & (1 << 10)) != 0 {
                        set_overflow(ctx, add_overflow(a, b, res));
                    }
                    if (word & 1) != 0 {
                        ctx.gekko.update_cr0(res);
                    }
                    true
                }
                // lwzux
                55 => {
                    let addr = ea_index(ctx, ra, rb);
                    let val = ctx.read_u32_interp(addr);
                    ctx.gekko.write_gpr(rd, val);
                    ctx.gekko.write_gpr(ra, addr);
                    true
                }
                // dcbst
                54 => true,
                // andc
                60 => {
                    let res = ctx.gekko.read_gpr(rs) & !ctx.gekko.read_gpr(rb);
                    ctx.gekko.write_gpr(ra, res);
                    if (word & 1) != 0 {
                        ctx.gekko.update_cr0(res);
                    }
                    true
                }
                // mulhw
                75 => {
                    let res =
                        ((ctx.gekko.read_gpr(ra) as i32 as i64 * ctx.gekko.read_gpr(rb) as i32 as i64) >> 32) as u32;
                    ctx.gekko.write_gpr(rd, res);
                    if (word & 1) != 0 {
                        ctx.gekko.update_cr0(res);
                    }
                    true
                }
                // mfmsr
                83 => {
                    ctx.gekko.write_gpr(rd, ctx.gekko.msr.raw());
                    true
                }
                // dcbf
                86 => true,
                // lbzx
                87 => {
                    let addr = ea_index(ctx, ra, rb);
                    let val = ctx.read_u8_interp(addr) as u32;
                    ctx.gekko.write_gpr(rd, val);
                    true
                }
                // neg
                104 => {
                    let a = ctx.gekko.read_gpr(ra);
                    let res = (!a).wrapping_add(1);
                    ctx.gekko.write_gpr(rd, res);
                    if (word & (1 << 10)) != 0 {
                        set_overflow(ctx, a == 0x8000_0000);
                    }
                    if (word & 1) != 0 {
                        ctx.gekko.update_cr0(res);
                    }
                    true
                }
                // lbzux
                119 => {
                    let addr = ea_index(ctx, ra, rb);
                    let val = ctx.read_u8_interp(addr) as u32;
                    ctx.gekko.write_gpr(rd, val);
                    ctx.gekko.write_gpr(ra, addr);
                    true
                }
                // nor
                124 => {
                    let res = !(ctx.gekko.read_gpr(rs) | ctx.gekko.read_gpr(rb));
                    ctx.gekko.write_gpr(ra, res);
                    if (word & 1) != 0 {
                        ctx.gekko.update_cr0(res);
                    }
                    true
                }
                // subfe
                136 => {
                    let ra_val = ctx.gekko.read_gpr(ra);
                    let rb_val = ctx.gekko.read_gpr(rb);
                    let ca = ctx.gekko.spr.xer.carry() as u32;
                    let (t1, c1) = (!ra_val).overflowing_add(rb_val);
                    let (res, c2) = t1.overflowing_add(ca);
                    ctx.gekko.write_gpr(rd, res);
                    ctx.gekko.spr.xer = ctx.gekko.spr.xer.with_carry(c1 || c2);
                    if (word & (1 << 10)) != 0 {
                        set_overflow(ctx, add_overflow(!ra_val, rb_val, res));
                    }
                    if (word & 1) != 0 {
                        ctx.gekko.update_cr0(res);
                    }
                    true
                }
                // adde
                138 => {
                    let ra_val = ctx.gekko.read_gpr(ra);
                    let rb_val = ctx.gekko.read_gpr(rb);
                    let ca = ctx.gekko.spr.xer.carry() as u32;
                    let (t1, c1) = ra_val.overflowing_add(rb_val);
                    let (res, c2) = t1.overflowing_add(ca);
                    ctx.gekko.write_gpr(rd, res);
                    ctx.gekko.spr.xer = ctx.gekko.spr.xer.with_carry(c1 || c2);
                    if (word & (1 << 10)) != 0 {
                        set_overflow(ctx, add_overflow(ra_val, rb_val, res));
                    }
                    if (word & 1) != 0 {
                        ctx.gekko.update_cr0(res);
                    }
                    true
                }
                // mtmsr
                146 => {
                    ctx.gekko.msr = crate::gekko::msr::Msr::from(ctx.gekko.read_gpr(rs));
                    true
                }
                // mtcrf
                144 => {
                    let crm = ((word >> 12) & 0xFF) as u8;
                    let rs_val = ctx.gekko.read_gpr(rs);
                    let mut cr = ctx.gekko.cr.raw();
                    for i in 0u8..8 {
                        if crm & (1 << (7 - i)) != 0 {
                            let shift = (7 - i) * 4;
                            let nibble_mask = 0xFu32 << shift;
                            cr = (cr & !nibble_mask) | (rs_val & nibble_mask);
                        }
                    }
                    ctx.gekko.cr = crate::gekko::condition::ConditionRegister::from(cr);
                    true
                }
                // stwx
                151 => {
                    let addr = ea_index(ctx, ra, rb);
                    ctx.write_u32_interp(addr, ctx.gekko.read_gpr(rs));
                    true
                }
                // stwux
                183 => {
                    let addr = ea_index(ctx, ra, rb);
                    ctx.write_u32_interp(addr, ctx.gekko.read_gpr(rs));
                    ctx.gekko.write_gpr(ra, addr);
                    true
                }
                // subfze
                200 => {
                    let ra_val = ctx.gekko.read_gpr(ra);
                    let ca = ctx.gekko.spr.xer.carry() as u32;
                    let (res, carry) = (!ra_val).overflowing_add(ca);
                    ctx.gekko.write_gpr(rd, res);
                    ctx.gekko.spr.xer = ctx.gekko.spr.xer.with_carry(carry);
                    if (word & (1 << 10)) != 0 {
                        set_overflow(ctx, add_overflow(!ra_val, 0, res));
                    }
                    if (word & 1) != 0 {
                        ctx.gekko.update_cr0(res);
                    }
                    true
                }
                // addze
                202 => {
                    let ra_val = ctx.gekko.read_gpr(ra);
                    let ca = ctx.gekko.spr.xer.carry() as u32;
                    let (res, carry) = ra_val.overflowing_add(ca);
                    ctx.gekko.write_gpr(rd, res);
                    ctx.gekko.spr.xer = ctx.gekko.spr.xer.with_carry(carry);
                    if (word & (1 << 10)) != 0 {
                        set_overflow(ctx, add_overflow(ra_val, 0, res));
                    }
                    if (word & 1) != 0 {
                        ctx.gekko.update_cr0(res);
                    }
                    true
                }
                // stbx
                215 => {
                    let addr = ea_index(ctx, ra, rb);
                    ctx.write_u8_interp(addr, ctx.gekko.read_gpr(rs) as u8);
                    true
                }
                // subfme
                232 => {
                    let ra_val = ctx.gekko.read_gpr(ra);
                    let ca = ctx.gekko.spr.xer.carry() as u32;
                    let (t1, c1) = (!ra_val).overflowing_add(ca);
                    let (res, c2) = t1.overflowing_add(0xFFFF_FFFF);
                    ctx.gekko.write_gpr(rd, res);
                    ctx.gekko.spr.xer = ctx.gekko.spr.xer.with_carry(c1 || c2);
                    if (word & (1 << 10)) != 0 {
                        set_overflow(ctx, add_overflow(!ra_val, 0xFFFF_FFFF, res));
                    }
                    if (word & 1) != 0 {
                        ctx.gekko.update_cr0(res);
                    }
                    true
                }
                // addme
                234 => {
                    let ra_val = ctx.gekko.read_gpr(ra);
                    let ca = ctx.gekko.spr.xer.carry() as u32;
                    let (t1, c1) = ra_val.overflowing_add(ca);
                    let (res, c2) = t1.overflowing_add(0xFFFF_FFFF);
                    ctx.gekko.write_gpr(rd, res);
                    ctx.gekko.spr.xer = ctx.gekko.spr.xer.with_carry(c1 || c2);
                    if (word & (1 << 10)) != 0 {
                        set_overflow(ctx, add_overflow(ra_val, 0xFFFF_FFFF, res));
                    }
                    if (word & 1) != 0 {
                        ctx.gekko.update_cr0(res);
                    }
                    true
                }
                // mullw
                235 => {
                    let full =
                        (ctx.gekko.read_gpr(ra) as i32 as i64).wrapping_mul(ctx.gekko.read_gpr(rb) as i32 as i64);
                    let res = full as u32;
                    ctx.gekko.write_gpr(rd, res);
                    if (word & (1 << 10)) != 0 {
                        set_overflow(ctx, full < i32::MIN as i64 || full > i32::MAX as i64);
                    }
                    if (word & 1) != 0 {
                        ctx.gekko.update_cr0(res);
                    }
                    true
                }
                // stbux
                247 => {
                    let addr = ea_index(ctx, ra, rb);
                    ctx.write_u8_interp(addr, ctx.gekko.read_gpr(rs) as u8);
                    ctx.gekko.write_gpr(ra, addr);
                    true
                }
                // add
                266 => {
                    let a = ctx.gekko.read_gpr(ra);
                    let b = ctx.gekko.read_gpr(rb);
                    let res = a.wrapping_add(b);
                    ctx.gekko.write_gpr(rd, res);
                    if (word & (1 << 10)) != 0 {
                        set_overflow(ctx, add_overflow(a, b, res));
                    }
                    if (word & 1) != 0 {
                        ctx.gekko.update_cr0(res);
                    }
                    true
                }
                // lhzx
                279 => {
                    let addr = ea_index(ctx, ra, rb);
                    let val = ctx.read_u16_interp(addr) as u32;
                    ctx.gekko.write_gpr(rd, val);
                    true
                }
                // eqv
                284 => {
                    let res = !(ctx.gekko.read_gpr(rs) ^ ctx.gekko.read_gpr(rb));
                    ctx.gekko.write_gpr(ra, res);
                    if (word & 1) != 0 {
                        ctx.gekko.update_cr0(res);
                    }
                    true
                }
                // lhzux
                311 => {
                    let addr = ea_index(ctx, ra, rb);
                    let val = ctx.read_u16_interp(addr) as u32;
                    ctx.gekko.write_gpr(rd, val);
                    ctx.gekko.write_gpr(ra, addr);
                    true
                }
                // xor
                316 => {
                    let res = ctx.gekko.read_gpr(rs) ^ ctx.gekko.read_gpr(rb);
                    ctx.gekko.write_gpr(ra, res);
                    if (word & 1) != 0 {
                        ctx.gekko.update_cr0(res);
                    }
                    true
                }
                // mfspr
                339 => {
                    let spr_raw = (word >> 11) & 0x3FF;
                    let spr_num = ((spr_raw >> 5) | ((spr_raw & 0x1F) << 5)) as u32;
                    let val = match spr_num {
                        22 => {
                            ctx.gekko.spr.dec = ctx.gekko.dec.read(ctx.scheduler.cycles);
                            ctx.gekko.spr.dec
                        }
                        268 => ctx.scheduler.timebase_lower(),
                        269 => ctx.scheduler.timebase_upper(),
                        _ => ctx.gekko.spr.read(spr_num),
                    };
                    ctx.gekko.write_gpr(rd, val);
                    true
                }
                // mftb
                371 => {
                    let tbr_raw = (word >> 11) & 0x3FF;
                    let tbr = ((tbr_raw >> 5) | ((tbr_raw & 0x1F) << 5)) as u32;
                    let val = match tbr {
                        268 => ctx.scheduler.timebase_lower(),
                        269 => ctx.scheduler.timebase_upper(),
                        _ => return false,
                    };
                    ctx.gekko.write_gpr(rd, val);
                    true
                }
                // lhax
                343 => {
                    let addr = ea_index(ctx, ra, rb);
                    let val = ctx.read_u16_interp(addr) as i16 as i32 as u32;
                    ctx.gekko.write_gpr(rd, val);
                    true
                }
                // lhaux
                375 => {
                    let addr = ea_index(ctx, ra, rb);
                    let val = ctx.read_u16_interp(addr) as i16 as i32 as u32;
                    ctx.gekko.write_gpr(rd, val);
                    ctx.gekko.write_gpr(ra, addr);
                    true
                }
                // sthx
                407 => {
                    let addr = ea_index(ctx, ra, rb);
                    ctx.write_u16_interp(addr, ctx.gekko.read_gpr(rs) as u16);
                    true
                }
                // orc
                412 => {
                    let res = ctx.gekko.read_gpr(rs) | !ctx.gekko.read_gpr(rb);
                    ctx.gekko.write_gpr(ra, res);
                    if (word & 1) != 0 {
                        ctx.gekko.update_cr0(res);
                    }
                    true
                }
                // sthux
                439 => {
                    let addr = ea_index(ctx, ra, rb);
                    ctx.write_u16_interp(addr, ctx.gekko.read_gpr(rs) as u16);
                    ctx.gekko.write_gpr(ra, addr);
                    true
                }
                // or
                444 => {
                    let res = ctx.gekko.read_gpr(rs) | ctx.gekko.read_gpr(rb);
                    ctx.gekko.write_gpr(ra, res);
                    if (word & 1) != 0 {
                        ctx.gekko.update_cr0(res);
                    }
                    true
                }
                // mtspr
                467 => {
                    let spr_raw = (word >> 11) & 0x3FF;
                    let spr_num = ((spr_raw >> 5) | ((spr_raw & 0x1F) << 5)) as u32;
                    let val = ctx.gekko.read_gpr(rs);
                    match spr_num {
                        22 => {
                            ctx.scheduler.cancel(crate::gekko::dec::underflow_handler::<SYSTEM>);
                            ctx.gekko.dec.write(ctx.scheduler.cycles, val);
                            ctx.gekko.spr.dec = val;
                            ctx.scheduler.schedule_in(
                                crate::gekko::dec::cycles_until_underflow(val),
                                crate::gekko::dec::underflow_handler::<SYSTEM>,
                            );
                        }
                        284 => ctx.scheduler.set_timebase_lower(val),
                        285 => ctx.scheduler.set_timebase_upper(val),
                        923 => {
                            ctx.gekko.spr.dmal = crate::gekko::spr::DmaLower::from_raw(val);
                            if ctx.gekko.spr.dmal.trigger() {
                                let dmau = ctx.gekko.spr.dmau;
                                let dmal = ctx.gekko.spr.dmal;
                                let written = ctx.mmio.process_locked_cache_dma(&dmau, &dmal);
                                #[cfg(feature = "jit")]
                                if let Some((phys, len)) = written {
                                    ctx.mmio.queue_icbi_for_range(phys, len);
                                }
                                #[cfg(not(feature = "jit"))]
                                let _ = written;
                            }
                        }
                        _ => ctx.gekko.spr.write(spr_num, val),
                    }
                    true
                }
                // dcbi
                470 => true,
                // nand
                476 => {
                    let res = !(ctx.gekko.read_gpr(rs) & ctx.gekko.read_gpr(rb));
                    ctx.gekko.write_gpr(ra, res);
                    if (word & 1) != 0 {
                        ctx.gekko.update_cr0(res);
                    }
                    true
                }
                // divwu
                459 => {
                    let ra_val = ctx.gekko.read_gpr(ra);
                    let rb_val = ctx.gekko.read_gpr(rb);
                    let res = if rb_val == 0 { 0 } else { ra_val / rb_val };
                    ctx.gekko.write_gpr(rd, res);
                    if (word & (1 << 10)) != 0 {
                        set_overflow(ctx, rb_val == 0);
                    }
                    if (word & 1) != 0 {
                        ctx.gekko.update_cr0(res);
                    }
                    true
                }
                // divw
                491 => {
                    let ra_val = ctx.gekko.read_gpr(ra) as i32;
                    let rb_val = ctx.gekko.read_gpr(rb) as i32;
                    let res = if rb_val == 0 || (ra_val == i32::MIN && rb_val == -1) {
                        if ra_val < 0 { u32::MAX } else { 0 }
                    } else {
                        (ra_val / rb_val) as u32
                    };
                    ctx.gekko.write_gpr(rd, res);
                    if (word & (1 << 10)) != 0 {
                        set_overflow(ctx, rb_val == 0 || (ra_val == i32::MIN && rb_val == -1));
                    }
                    if (word & 1) != 0 {
                        ctx.gekko.update_cr0(res);
                    }
                    true
                }
                // sync
                598 => true,
                // lfsx
                535 => {
                    if !ctx.check_fp_available() {
                        return true;
                    }
                    let addr = ea_index(ctx, ra, rb);
                    let val = ctx.read_f32(addr);
                    ctx.gekko.write_fpr(rd, val);
                    ctx.gekko.write_ps1(rd, val);
                    true
                }
                // srw
                536 => {
                    let rs_val = ctx.gekko.read_gpr(rs);
                    let rb_val = ctx.gekko.read_gpr(rb);
                    let sh = rb_val & 0x3F;
                    let res = if sh >= 32 { 0 } else { rs_val >> sh };
                    ctx.gekko.write_gpr(ra, res);
                    if (word & 1) != 0 {
                        ctx.gekko.update_cr0(res);
                    }
                    true
                }
                // stfsx
                663 => {
                    if !ctx.check_fp_available() {
                        return true;
                    }
                    let addr = ea_index(ctx, ra, rb);
                    ctx.write_f32(addr, ctx.gekko.read_fpr(rs));
                    true
                }
                // sraw
                792 => {
                    let rs_val = ctx.gekko.read_gpr(rs);
                    let rb_val = ctx.gekko.read_gpr(rb);
                    let sh = rb_val & 0x3F;
                    let signed = rs_val as i32;
                    let res = if sh >= 32 {
                        ctx.gekko.spr.xer = ctx.gekko.spr.xer.with_carry(signed < 0);
                        (signed >> 31) as u32
                    } else if sh == 0 {
                        ctx.gekko.spr.xer = ctx.gekko.spr.xer.with_carry(false);
                        rs_val
                    } else {
                        let mask = (1u32 << sh) - 1;
                        ctx.gekko.spr.xer = ctx.gekko.spr.xer.with_carry(signed < 0 && (rs_val & mask) != 0);
                        (signed >> sh) as u32
                    };
                    ctx.gekko.write_gpr(ra, res);
                    if (word & 1) != 0 {
                        ctx.gekko.update_cr0(res);
                    }
                    true
                }
                // srawi
                824 => {
                    let rs_val = ctx.gekko.read_gpr(rs);
                    let sh = ((word >> 11) & 0x1F) as u32;
                    let signed = rs_val as i32;
                    let res = if sh == 0 {
                        ctx.gekko.spr.xer = ctx.gekko.spr.xer.with_carry(false);
                        rs_val
                    } else {
                        let mask = (1u32 << sh) - 1;
                        ctx.gekko.spr.xer = ctx.gekko.spr.xer.with_carry(signed < 0 && (rs_val & mask) != 0);
                        (signed >> sh) as u32
                    };
                    ctx.gekko.write_gpr(ra, res);
                    if (word & 1) != 0 {
                        ctx.gekko.update_cr0(res);
                    }
                    true
                }
                // extsh
                922 => {
                    let res = ctx.gekko.read_gpr(rs) as i16 as i32 as u32;
                    ctx.gekko.write_gpr(ra, res);
                    if (word & 1) != 0 {
                        ctx.gekko.update_cr0(res);
                    }
                    true
                }
                // extsb
                954 => {
                    let res = ctx.gekko.read_gpr(rs) as i8 as i32 as u32;
                    ctx.gekko.write_gpr(ra, res);
                    if (word & 1) != 0 {
                        ctx.gekko.update_cr0(res);
                    }
                    true
                }
                _ => false,
            }
        }
        // lwz
        32 => {
            let addr = ea_disp(ctx, ra, low_s16(word));
            let val = ctx.read_u32_interp(addr);
            ctx.gekko.write_gpr(rd, val);
            true
        }
        // lwzu
        33 => {
            let addr = ea_disp(ctx, ra, low_s16(word));
            let val = ctx.read_u32_interp(addr);
            ctx.gekko.write_gpr(rd, val);
            ctx.gekko.write_gpr(ra, addr);
            true
        }
        // lbz
        34 => {
            let addr = ea_disp(ctx, ra, low_s16(word));
            let val = ctx.read_u8_interp(addr) as u32;
            ctx.gekko.write_gpr(rd, val);
            true
        }
        // lbzu
        35 => {
            let addr = ea_disp(ctx, ra, low_s16(word));
            let val = ctx.read_u8_interp(addr) as u32;
            ctx.gekko.write_gpr(rd, val);
            ctx.gekko.write_gpr(ra, addr);
            true
        }
        // stw
        36 => {
            let addr = ea_disp(ctx, ra, low_s16(word));
            ctx.write_u32_interp(addr, ctx.gekko.read_gpr(rs));
            true
        }
        // stwu
        37 => {
            let addr = ea_disp(ctx, ra, low_s16(word));
            ctx.write_u32_interp(addr, ctx.gekko.read_gpr(rs));
            ctx.gekko.write_gpr(ra, addr);
            true
        }
        // stb
        38 => {
            let addr = ea_disp(ctx, ra, low_s16(word));
            ctx.write_u8_interp(addr, ctx.gekko.read_gpr(rs) as u8);
            true
        }
        // stbu
        39 => {
            let addr = ea_disp(ctx, ra, low_s16(word));
            ctx.write_u8_interp(addr, ctx.gekko.read_gpr(rs) as u8);
            ctx.gekko.write_gpr(ra, addr);
            true
        }
        // lhz
        40 => {
            let addr = ea_disp(ctx, ra, low_s16(word));
            let val = ctx.read_u16_interp(addr) as u32;
            ctx.gekko.write_gpr(rd, val);
            true
        }
        // lhzu
        41 => {
            let addr = ea_disp(ctx, ra, low_s16(word));
            let val = ctx.read_u16_interp(addr) as u32;
            ctx.gekko.write_gpr(rd, val);
            ctx.gekko.write_gpr(ra, addr);
            true
        }
        // lha
        42 => {
            let addr = ea_disp(ctx, ra, low_s16(word));
            let val = ctx.read_u16_interp(addr) as i16 as i32 as u32;
            ctx.gekko.write_gpr(rd, val);
            true
        }
        // lhau
        43 => {
            let addr = ea_disp(ctx, ra, low_s16(word));
            let val = ctx.read_u16_interp(addr) as i16 as i32 as u32;
            ctx.gekko.write_gpr(rd, val);
            ctx.gekko.write_gpr(ra, addr);
            true
        }
        // sth
        44 => {
            let addr = ea_disp(ctx, ra, low_s16(word));
            ctx.write_u16_interp(addr, ctx.gekko.read_gpr(rs) as u16);
            true
        }
        // sthu
        45 => {
            let addr = ea_disp(ctx, ra, low_s16(word));
            ctx.write_u16_interp(addr, ctx.gekko.read_gpr(rs) as u16);
            ctx.gekko.write_gpr(ra, addr);
            true
        }
        // lmw
        46 => {
            let mut addr = ea_disp(ctx, ra, low_s16(word));
            for r in rd..32 {
                let val = ctx.read_u32_interp(addr);
                ctx.gekko.write_gpr(r, val);
                addr = addr.wrapping_add(4);
            }
            true
        }
        // stmw
        47 => {
            let mut addr = ea_disp(ctx, ra, low_s16(word));
            for r in rs..32 {
                let val = ctx.gekko.read_gpr(r);
                ctx.write_u32_interp(addr, val);
                addr = addr.wrapping_add(4);
            }
            true
        }
        _ => false,
    }
}
