use std::collections::VecDeque;

use rustc_hash::FxHashMap;

use crate::gekko::instruction::Instruction;
use crate::system::{System, SystemId};

const HOT_THRESHOLD: u16 = 16;
const MAX_BLOCK_LEN: usize = 24;
const MAX_BLOCK_CACHE_ENTRIES: usize = 4096;

#[derive(Clone, Copy)]
struct CachedInsn {
    pc: u32,
    raw: u32,
    instr: Instruction,
}

struct CachedBlock {
    instructions: Vec<CachedInsn>,
}

pub struct Jiterpreter<const SYSTEM: SystemId> {
    hot_counts: FxHashMap<u32, u16>,
    blocks: FxHashMap<u32, CachedBlock>,
    insertion_order: VecDeque<u32>,
    _marker: core::marker::PhantomData<System<SYSTEM>>,
}

impl<const SYSTEM: SystemId> Default for Jiterpreter<SYSTEM> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const SYSTEM: SystemId> Jiterpreter<SYSTEM> {
    pub fn new() -> Self {
        Self {
            hot_counts: FxHashMap::default(),
            blocks: FxHashMap::default(),
            insertion_order: VecDeque::new(),
            _marker: core::marker::PhantomData,
        }
    }

    pub fn run_block(&mut self, sys: &mut System<SYSTEM>) {
        #[cfg(feature = "hooks")]
        {
            sys.step_cpu();
            return;
        }

        let start_pc = sys.gekko.pc;

        if self.execute_cached_if_available(sys, start_pc) {
            return;
        }

        self.run_fallback(sys);
        self.on_fallback(start_pc, sys);
    }

    fn execute_cached_if_available(&mut self, sys: &mut System<SYSTEM>, start_pc: u32) -> bool {
        let Some(block) = self.blocks.get(&start_pc) else {
            return false;
        };

        if self.execute_cached_block(sys, block) {
            true
        } else {
            self.blocks.remove(&start_pc);
            false
        }
    }

    fn execute_cached_block(&self, sys: &mut System<SYSTEM>, block: &CachedBlock) -> bool {
        if block.instructions.is_empty() {
            return false;
        }

        for cached in &block.instructions {
            if sys.gekko.pc != cached.pc {
                return false;
            }

            let raw = sys.mmio.fetch_instruction(cached.pc);
            if raw != cached.raw {
                return false;
            }

            sys.gekko.cia = cached.pc;
            sys.gekko.nia = cached.pc.wrapping_add(4);
            crate::gekko::dispatch(sys, cached.instr);
            sys.scheduler.cycles += 2;
            sys.gekko.pc = sys.gekko.nia;
        }

        true
    }

    fn run_fallback(&self, sys: &mut System<SYSTEM>) {
        sys.step_cpu();
    }

    fn on_fallback(&mut self, start_pc: u32, sys: &System<SYSTEM>) {
        let hot = self.hot_counts.entry(start_pc).or_insert(0);
        *hot = hot.saturating_add(1);

        if *hot >= HOT_THRESHOLD && !self.blocks.contains_key(&start_pc) {
            let block = Self::build_block(sys, start_pc);
            self.insert_block(start_pc, block);
        }
    }

    fn insert_block(&mut self, start_pc: u32, block: CachedBlock) {
        if block.instructions.is_empty() {
            return;
        }

        if self.blocks.insert(start_pc, block).is_none() {
            self.insertion_order.push_back(start_pc);
        }

        while self.blocks.len() > MAX_BLOCK_CACHE_ENTRIES {
            let Some(evict_pc) = self.insertion_order.pop_front() else {
                break;
            };
            self.blocks.remove(&evict_pc);
        }
    }

    fn build_block(sys: &System<SYSTEM>, start_pc: u32) -> CachedBlock {
        let mut instructions = Vec::with_capacity(MAX_BLOCK_LEN);
        let mut pc = start_pc;
        for _ in 0..MAX_BLOCK_LEN {
            let raw = sys.mmio.fetch_instruction(pc);
            let instr = Instruction(raw);
            instructions.push(CachedInsn { pc, raw, instr });

            if Self::is_terminator(raw) {
                break;
            }

            pc = pc.wrapping_add(4);
        }
        CachedBlock { instructions }
    }

    fn is_terminator(raw: u32) -> bool {
        let opcd = (raw >> 26) as u8;
        matches!(opcd, 16 | 17 | 18 | 19)
    }
}
