use std::collections::{BTreeMap, VecDeque};

use rustc_hash::{FxHashMap, FxHashSet};

use crate::gekko::instruction::Instruction;
use crate::jit_cache::hash_words;
use crate::mmio::constants::RAM_END;
use crate::mmio::virt_to_phys;
use crate::mmio::CODE_LINE_SHIFT;
use crate::system::SystemId;
#[cfg(target_arch = "wasm32")]
use std::sync::atomic::{AtomicUsize, Ordering};
#[cfg(target_arch = "wasm32")]
use js_sys::Uint32Array;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::closure::Closure;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

pub const HOT_BLOCK_THRESHOLD: u32 = 32;
const MIN_COMPILED_BLOCK_INSTRS: usize = 6;
const MAX_TRACE_BLOCKS: usize = 8;
const MAX_TRACE_INSTRS: usize = 64;
const DISPATCH_BUCKETS: usize = 32;
const DISPATCH_BUCKET_MASK: u32 = (DISPATCH_BUCKETS as u32) - 1;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(inline_js = r#"
export function geckoRuntimeWasmCompile(bytes, readU32, readU16, readU8, writeU32, writeU16, writeU8, memory, statePtr) {
    const module = new WebAssembly.Module(bytes);
    const instance = new WebAssembly.Instance(module, {
        env: {
            read_u32: readU32,
            read_u16: readU16,
            read_u8: readU8,
            write_u32: writeU32,
            write_u16: writeU16,
            write_u8: writeU8,
            memory,
        },
    });
    return {
        run: instance.exports.run,
        memory,
        statePtr,
        state: new Uint32Array(memory.buffer, statePtr, 41),
    };
}

export function geckoRuntimeWasmInvoke(executor, state) {
    let view = executor.state;
    if (view.buffer !== executor.memory.buffer) {
        view = new Uint32Array(executor.memory.buffer, executor.statePtr, 41);
        executor.state = view;
    }
    view.set(state);
    executor.run();
    return view;
}
"#)]
extern "C" {
    #[wasm_bindgen(catch, js_name = geckoRuntimeWasmCompile)]
    fn gecko_runtime_wasm_compile(
        bytes: &[u8],
        read_u32: &JsValue,
        read_u16: &JsValue,
        read_u8: &JsValue,
        write_u32: &JsValue,
        write_u16: &JsValue,
        write_u8: &JsValue,
        memory: &JsValue,
        state_ptr: u32,
    ) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = geckoRuntimeWasmInvoke)]
    fn gecko_runtime_wasm_invoke(run: &JsValue, state: &[u32]) -> Uint32Array;
}

const GPR_PARAM_COUNT: u32 = 32;
const CR_PARAM_INDEX: u32 = 32;
const CTR_PARAM_INDEX: u32 = 33;
const SO_PARAM_INDEX: u32 = 34;
const OV_PARAM_INDEX: u32 = 35;
const CA_PARAM_INDEX: u32 = 36;
const PC_STATE_INDEX: u32 = 37;
const LAST_PC_STATE_INDEX: u32 = 38;
const LR_STATE_INDEX: u32 = 39;
const EXECUTED_INSTRS_STATE_INDEX: u32 = 40;
const STATE_WORDS: usize = 41;
const CURRENT_PC_LOCAL_INDEX: u32 = 37;
const LAST_PC_LOCAL_INDEX: u32 = 38;
const NEXT_PC_LOCAL_INDEX: u32 = 39;
const LR_LOCAL_INDEX: u32 = 40;
const EXECUTED_INSTRS_LOCAL_INDEX: u32 = 41;
const CMP_LHS_LOCAL_INDEX: u32 = 42;
const CMP_RHS_LOCAL_INDEX: u32 = 43;

const MAX_BLOCK_INSTRS: usize = 256;
const READ_U32_IMPORT_INDEX: u32 = 0;
const READ_U16_IMPORT_INDEX: u32 = 1;
const READ_U8_IMPORT_INDEX: u32 = 2;
const WRITE_U32_IMPORT_INDEX: u32 = 3;
const WRITE_U16_IMPORT_INDEX: u32 = 4;
const WRITE_U8_IMPORT_INDEX: u32 = 5;
const READ_SCALAR_FUNCTION_TYPE_INDEX: u32 = 0;
const WRITE_SCALAR_FUNCTION_TYPE_INDEX: u32 = 1;
const RUN_FUNCTION_TYPE_INDEX: u32 = 2;
const RUN_FUNCTION_INDEX: u32 = 6;

#[cfg(target_arch = "wasm32")]
static ACTIVE_RUNTIME_WASM_SYSTEM_GC: AtomicUsize = AtomicUsize::new(0);
#[cfg(target_arch = "wasm32")]
static ACTIVE_RUNTIME_WASM_SYSTEM_WII: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TermKind {
    Branch,
    BranchLink,
    BranchCond,
    BranchToReg,
    SystemCall,
    Rfi,
    Mtmsr,
    Mtspr,
    Isync,
    LengthCap,
}

#[derive(Debug, Clone)]
pub struct BlockSpec {
    pub start_pc: u32,
    pub instrs: Vec<u32>,
    pub pcs: Vec<u32>,
    pub terminator: TermKind,
}

#[derive(Debug, Clone)]
pub struct TraceSpec {
    pub entry: BlockSpec,
    pub successors: Vec<BlockSpec>,
}

impl TraceSpec {
    #[inline(always)]
    pub fn total_instrs(&self) -> usize {
        self.entry.len() + self.successors.iter().map(BlockSpec::len).sum::<usize>()
    }

    #[inline(always)]
    pub fn all_blocks(&self) -> impl Iterator<Item = &BlockSpec> {
        std::iter::once(&self.entry).chain(self.successors.iter())
    }
}

impl BlockSpec {
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.instrs.len()
    }

    #[inline(always)]
    pub fn end_pc(&self) -> u32 {
        self.pcs
            .last()
            .copied()
            .map(|p| p.wrapping_add(4))
            .unwrap_or(self.start_pc)
    }
}

#[inline(always)]
fn primary_opcode(instr: Instruction) -> u8 {
    (instr.0 >> 26) as u8
}

#[inline(always)]
fn xo10(instr: Instruction) -> u32 {
    (instr.0 >> 1) & 0x3ff
}

#[inline]
fn extension_target(instr: Instruction, pc: u32) -> Option<u32> {
    if primary_opcode(instr) != 18 || instr.lk() {
        return None;
    }

    Some(if instr.aa() {
        instr.li() as u32
    } else {
        pc.wrapping_add_signed(instr.li())
    })
}

#[inline]
fn mtspr_is_block_safe(spr: u16) -> bool {
    matches!(spr, 1 | 8 | 9 | 22 | 26 | 27 | 272..=275 | 912..=919 | 920 | 1008 | 1009)
}

#[inline]
fn classify_terminator(instr: Instruction) -> Option<TermKind> {
    match primary_opcode(instr) {
        16 => Some(TermKind::BranchCond),
        17 => Some(TermKind::SystemCall),
        18 => Some(if instr.lk() { TermKind::BranchLink } else { TermKind::Branch }),
        19 => match xo10(instr) {
            16 | 528 => Some(TermKind::BranchToReg),
            50 => Some(TermKind::Rfi),
            _ => None,
        },
        31 => match xo10(instr) {
            146 => Some(TermKind::Mtmsr),
            467 => {
                let spr_num = instr.spr_swapped() as u16;
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

pub(crate) fn discover_block<const SYSTEM: SystemId>(sys: &crate::system::System<SYSTEM>, start_pc: u32) -> BlockSpec {
    const EXTENSION_MAX_FORWARD_BYTES: u32 = 1024;

    let mut instrs: Vec<u32> = Vec::with_capacity(8);
    let mut pcs: Vec<u32> = Vec::with_capacity(8);
    let mut terminator = TermKind::LengthCap;
    let mut pc = start_pc;

    while instrs.len() < MAX_BLOCK_INSTRS {
        let instr = Instruction(sys.mmio.fetch_instruction(pc));
        let cur_pc = pc;

        if let Some(target) = extension_target(instr, cur_pc) {
            if target > cur_pc && target.wrapping_sub(cur_pc) <= EXTENSION_MAX_FORWARD_BYTES && !pcs.contains(&target) {
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

    BlockSpec {
        start_pc,
        instrs,
        pcs,
        terminator,
    }
}

pub(crate) fn discover_trace<const SYSTEM: SystemId>(sys: &crate::system::System<SYSTEM>, start_pc: u32) -> TraceSpec {
    let entry = discover_block(sys, start_pc);
    let mut successors = Vec::new();
    let mut seen = FxHashSet::default();
    let mut queue = VecDeque::new();
    let mut total_instrs = entry.len();

    seen.insert(entry.start_pc);
    for succ_pc in block_successors(&entry) {
        if seen.insert(succ_pc) {
            queue.push_back(succ_pc);
        }
    }

    while let Some(succ_pc) = queue.pop_front() {
        if successors.len() >= MAX_TRACE_BLOCKS.saturating_sub(1) {
            break;
        }

        let succ = discover_block(sys, succ_pc);
        if total_instrs + succ.len() > MAX_TRACE_INSTRS {
            continue;
        }

        total_instrs += succ.len();
        for next_pc in block_successors(&succ) {
            if seen.insert(next_pc) {
                queue.push_back(next_pc);
            }
        }
        successors.push(succ);
    }

    TraceSpec { entry, successors }
}

fn block_successors(spec: &BlockSpec) -> Vec<u32> {
    let Some(&raw) = spec.instrs.last() else {
        return Vec::new();
    };
    let Some(&pc) = spec.pcs.last() else {
        return Vec::new();
    };
    let instr = Instruction(raw);

    match spec.terminator {
        TermKind::LengthCap => vec![spec.end_pc()],
        TermKind::Branch => {
            let target = if instr.aa() {
                instr.li() as u32
            } else {
                pc.wrapping_add_signed(instr.li())
            };
            vec![target]
        }
        TermKind::BranchLink => Vec::new(),
        TermKind::BranchCond => {
            let target = if instr.aa() {
                instr.bd() as u32
            } else {
                pc.wrapping_add_signed(instr.bd())
            };
            vec![pc.wrapping_add(4), target]
        }
        _ => Vec::new(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnsupportedOpcode {
    Primary(u8),
    Xo10(u32),
    Mtspr(u16),
    Terminator(TermKind),
    Unprofitable { instr_count: u16 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlockCompileDecision {
    Compileable(BlockFingerprint),
    Fallback { pc: u32, reason: UnsupportedOpcode },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuntimeWasmStats {
    pub compiled_executions: u64,
    pub compiled_execution_instrs: u64,
    pub compile_attempts: u64,
    pub compiled_blocks: usize,
    pub unprofitable_fallbacks: u64,
    pub unsupported_fallbacks: u64,
    pub shared_rebuilds: u64,
    pub shared_dispatch_blocks: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlockFingerprint {
    pub pc: u32,
    pub instr_count: u16,
    pub hash: u64,
}

#[derive(Clone)]
pub struct CompiledBlock {
    pub fingerprint: BlockFingerprint,
    pub compiled_at_cycle: u64,
    pub hit_count: u32,
    pub trace: TraceSpec,
}

impl core::fmt::Debug for CompiledBlock {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CompiledBlock")
            .field("fingerprint", &self.fingerprint)
            .field("compiled_at_cycle", &self.compiled_at_cycle)
            .field("hit_count", &self.hit_count)
            .finish()
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Default)]
struct SharedContainer {
    runner: Option<JsValue>,
    dirty: bool,
    rebuilds: u64,
    dispatch_blocks: usize,
    read_u32_import: Option<Closure<dyn FnMut(u32) -> u32>>,
    read_u16_import: Option<Closure<dyn FnMut(u32) -> u32>>,
    read_u8_import: Option<Closure<dyn FnMut(u32) -> u32>>,
    write_u32_import: Option<Closure<dyn FnMut(u32, u32)>>,
    write_u16_import: Option<Closure<dyn FnMut(u32, u32)>>,
    write_u8_import: Option<Closure<dyn FnMut(u32, u32)>>,
    imported_memory: Option<JsValue>,
    state_storage: Box<[u32]>,
    mem1_base: u32,
    code_refcount_base: u32,
}

#[derive(Debug)]
pub struct RuntimeWasmState<const SYSTEM: SystemId> {
    hot_counts: FxHashMap<u32, u32>,
    hot_candidates: FxHashSet<u32>,
    fallback_counts: FxHashMap<u32, u64>,
    fallback_reason_counts: FxHashMap<UnsupportedOpcode, u64>,
    block_specs: FxHashMap<BlockFingerprint, BlockSpec>,
    pc_index: FxHashMap<u32, BlockFingerprint>,
    compiled_blocks: FxHashMap<BlockFingerprint, CompiledBlock>,
    invalidated_pcs: FxHashSet<u32>,
    begin_cycle: u64,
    begin_deadline: u64,
    compile_attempts: u64,
    compiled_executions: u64,
    compiled_execution_instrs: u64,
    unprofitable_fallbacks: u64,
    unsupported_fallbacks: u64,
    #[cfg(target_arch = "wasm32")]
    shared: SharedContainer,
}

impl<const SYSTEM: SystemId> RuntimeWasmState<SYSTEM> {
    pub fn new() -> Self {
        Self {
            hot_counts: FxHashMap::default(),
            hot_candidates: FxHashSet::default(),
            fallback_counts: FxHashMap::default(),
            fallback_reason_counts: FxHashMap::default(),
            block_specs: FxHashMap::default(),
            pc_index: FxHashMap::default(),
            compiled_blocks: FxHashMap::default(),
            invalidated_pcs: FxHashSet::default(),
            begin_cycle: 0,
            begin_deadline: 0,
            compile_attempts: 0,
            compiled_executions: 0,
            compiled_execution_instrs: 0,
            unprofitable_fallbacks: 0,
            unsupported_fallbacks: 0,
            #[cfg(target_arch = "wasm32")]
            shared: SharedContainer {
                runner: None,
                dirty: true,
                rebuilds: 0,
                dispatch_blocks: 0,
                read_u32_import: None,
                read_u16_import: None,
                read_u8_import: None,
                write_u32_import: None,
                write_u16_import: None,
                write_u8_import: None,
                imported_memory: None,
                state_storage: vec![0u32; STATE_WORDS].into_boxed_slice(),
                mem1_base: 0,
                code_refcount_base: 0,
            },
        }
    }

    pub fn begin_slice(&mut self, cycle: u64, deadline: u64) {
        self.begin_cycle = cycle;
        self.begin_deadline = deadline;
    }

    pub fn end_slice(&mut self, _cycle: u64) {}

    pub fn record_block_hit(&mut self, pc: u32) -> u32 {
        let count = self.hot_counts.entry(pc).or_insert(0);
        *count = count.saturating_add(1);
        if *count == HOT_BLOCK_THRESHOLD {
            self.hot_candidates.insert(pc);
        }
        *count
    }

    pub fn poll_hot_candidate(&mut self) -> Option<u32> {
        let pc = self.hot_candidates.iter().copied().next()?;
        self.hot_candidates.remove(&pc);
        Some(pc)
    }

    pub fn clear_hot_candidate(&mut self, pc: u32) {
        self.hot_candidates.remove(&pc);
    }

    pub fn classify_block(&self, spec: &BlockSpec) -> BlockCompileDecision {
        if let Some(reason) = self.block_unsupported_reason(spec) {
            return BlockCompileDecision::Fallback {
                pc: spec.start_pc,
                reason,
            };
        }

        let fingerprint = self.fingerprint(spec.start_pc, spec.len() as u16, &spec.instrs);
        BlockCompileDecision::Compileable(fingerprint)
    }

    pub fn note_fallback(&mut self, pc: u32, reason: UnsupportedOpcode) {
        let count = self.fallback_counts.entry(pc).or_insert(0);
        *count = count.saturating_add(1);
        let reason_count = self.fallback_reason_counts.entry(reason).or_insert(0);
        *reason_count = reason_count.saturating_add(1);

        match reason {
            UnsupportedOpcode::Unprofitable { .. } => {
                self.unprofitable_fallbacks = self.unprofitable_fallbacks.saturating_add(1);
            }
            _ => {
                self.unsupported_fallbacks = self.unsupported_fallbacks.saturating_add(1);
            }
        }

        tracing::debug!(
            pc = format!("{pc:08X}"),
            fallback_count = *count,
            reason = ?reason,
            "runtime wasm falling back to interpreter"
        );
    }

    pub fn discover_and_classify(
        &self,
        sys: &crate::system::System<SYSTEM>,
        start_pc: u32,
    ) -> BlockCompileDecision {
        let trace = discover_trace(sys, start_pc);
        self.classify_trace(&trace)
    }

    pub fn compile_candidate(
        &mut self,
        sys: &crate::system::System<SYSTEM>,
        start_pc: u32,
    ) -> BlockCompileDecision {
        let trace = discover_trace(sys, start_pc);
        self.compile_trace(trace)
    }

    pub fn compile_spec(&mut self, spec: BlockSpec) -> BlockCompileDecision {
        self.compile_trace(TraceSpec {
            entry: spec,
            successors: Vec::new(),
        })
    }

    pub fn classify_trace(&self, trace: &TraceSpec) -> BlockCompileDecision {
        let total_instrs = trace.total_instrs();
        if total_instrs < MIN_COMPILED_BLOCK_INSTRS {
            return BlockCompileDecision::Fallback {
                pc: trace.entry.start_pc,
                reason: UnsupportedOpcode::Unprofitable {
                    instr_count: total_instrs as u16,
                },
            };
        }

        let memory_instrs = self.trace_memory_op_count(trace);
        if total_instrs < 10 && memory_instrs * 2 >= total_instrs {
            return BlockCompileDecision::Fallback {
                pc: trace.entry.start_pc,
                reason: UnsupportedOpcode::Unprofitable {
                    instr_count: total_instrs as u16,
                },
            };
        }

        if let Some(reason) = self.block_semantic_unsupported_reason(&trace.entry) {
            return BlockCompileDecision::Fallback {
                pc: trace.entry.start_pc,
                reason,
            };
        }

        for succ in &trace.successors {
            if let Some(reason) = self.block_semantic_unsupported_reason(succ) {
                return BlockCompileDecision::Fallback {
                    pc: succ.start_pc,
                    reason,
                };
            }
        }

        let mut words = trace.entry.instrs.clone();
        for succ in &trace.successors {
            words.extend_from_slice(&succ.instrs);
        }
        let fingerprint = self.fingerprint(trace.entry.start_pc, total_instrs as u16, &words);
        BlockCompileDecision::Compileable(fingerprint)
    }

    fn trace_memory_op_count(&self, trace: &TraceSpec) -> usize {
        let mut count = 0;
        for raw in &trace.entry.instrs {
            count += usize::from(matches!(
                primary_opcode(Instruction(*raw)),
                32 | 33 | 34 | 35 | 36 | 37 | 38 | 39 | 40 | 41 | 42 | 43 | 44 | 45
            ));
        }
        for succ in &trace.successors {
            for raw in &succ.instrs {
                count += usize::from(matches!(
                    primary_opcode(Instruction(*raw)),
                    32 | 33 | 34 | 35 | 36 | 37 | 38 | 39 | 40 | 41 | 42 | 43 | 44 | 45
                ));
            }
        }
        count
    }

    pub fn compile_trace(&mut self, trace: TraceSpec) -> BlockCompileDecision {
        match self.classify_trace(&trace) {
            BlockCompileDecision::Compileable(fingerprint) => {
                self.block_specs.insert(fingerprint, trace.entry.clone());
                self.register_compiled_block(fingerprint, trace);
                BlockCompileDecision::Compileable(fingerprint)
            }
            fallback => fallback,
        }
    }

    fn block_semantic_unsupported_reason(&self, spec: &BlockSpec) -> Option<UnsupportedOpcode> {
        if spec.instrs.is_empty() {
            return Some(UnsupportedOpcode::Terminator(TermKind::LengthCap));
        }

        let last_idx = spec.instrs.len() - 1;
        for (idx, raw) in spec.instrs.iter().enumerate() {
            let instr = Instruction(*raw);
            let is_last = idx == last_idx;
            if is_last {
                if let Some(reason) = self.terminator_unsupported_reason(spec, instr) {
                    return Some(reason);
                }
                continue;
            }

            if let Some(reason) = self.instruction_unsupported_reason(instr) {
                return Some(reason);
            }
        }

        None
    }

    pub fn should_compile(&self, pc: u32) -> bool {
        self.hot_counts.get(&pc).copied().unwrap_or(0) >= HOT_BLOCK_THRESHOLD
    }

    pub fn fingerprint(&self, pc: u32, instr_count: u16, block_words: &[u32]) -> BlockFingerprint {
        BlockFingerprint {
            pc,
            instr_count,
            hash: hash_words(block_words.iter().copied().chain([pc, instr_count as u32, SYSTEM as u32])),
        }
    }

    pub fn lookup(&self, fingerprint: &BlockFingerprint) -> Option<&CompiledBlock> {
        if self.invalidated_pcs.contains(&fingerprint.pc) {
            return None;
        }
        self.compiled_blocks.get(fingerprint)
    }

    pub fn register_compiled_block(&mut self, fingerprint: BlockFingerprint, trace: TraceSpec) {
        self.compile_attempts = self.compile_attempts.saturating_add(1);
        self.invalidated_pcs.remove(&fingerprint.pc);
        for block in trace.all_blocks() {
            self.invalidated_pcs.remove(&block.start_pc);
            self.pc_index.insert(block.start_pc, fingerprint);
        }
        self.compiled_blocks.insert(
            fingerprint,
            CompiledBlock {
                fingerprint,
                compiled_at_cycle: self.begin_cycle,
                hit_count: self.hot_counts.get(&fingerprint.pc).copied().unwrap_or(0),
                trace,
            },
        );
        #[cfg(target_arch = "wasm32")]
        {
            self.shared.dirty = true;
            self.shared.runner = None;
        }
    }

    pub fn invalidate_pc(&mut self, pc: u32) {
        self.invalidated_pcs.insert(pc);
        let retired: Vec<_> = self
            .compiled_blocks
            .iter()
            .filter(|(_, block)| block.trace.all_blocks().any(|block_spec| block_spec.start_pc == pc))
            .map(|(fingerprint, _)| *fingerprint)
            .collect();

        for fingerprint in retired {
            if let Some(block) = self.compiled_blocks.remove(&fingerprint) {
                for block_spec in block.trace.all_blocks() {
                    self.invalidated_pcs.insert(block_spec.start_pc);
                    self.pc_index.remove(&block_spec.start_pc);
                }
            }
            self.block_specs.remove(&fingerprint);
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.shared.dirty = true;
            self.shared.runner = None;
        }
    }

    pub fn invalidate_phys_line(&mut self, phys_line: u32) {
        let retired: Vec<_> = self
            .compiled_blocks
            .iter()
            .filter(|(_, block)| {
                block
                    .trace
                    .all_blocks()
                    .any(|block_spec| virt_to_phys(block_spec.start_pc) == phys_line)
            })
            .map(|(fingerprint, _)| *fingerprint)
            .collect();

        for fingerprint in retired {
            if let Some(block) = self.compiled_blocks.remove(&fingerprint) {
                for block_spec in block.trace.all_blocks() {
                    self.pc_index.remove(&block_spec.start_pc);
                    self.invalidated_pcs.insert(block_spec.start_pc);
                }
            }
            self.block_specs.remove(&fingerprint);
        }
        self.invalidated_pcs.insert(phys_line);
        #[cfg(target_arch = "wasm32")]
        {
            self.shared.dirty = true;
            self.shared.runner = None;
        }
    }

    pub fn invalidate_range(&mut self, start_pc: u32, end_pc: u32) {
        let mut pcs = Vec::new();
        for fingerprint in self.compiled_blocks.keys() {
            if (start_pc..end_pc).contains(&fingerprint.pc) {
                pcs.push(fingerprint.pc);
            }
        }

        for pc in pcs {
            self.invalidate_pc(pc);
        }
    }

    pub fn take_invalidated_pcs(&mut self) -> Vec<u32> {
        self.invalidated_pcs.drain().collect()
    }

    pub fn compile_attempts(&self) -> u64 {
        self.compile_attempts
    }

    pub fn note_compiled_execution(&mut self, instr_count: u64) {
        self.compiled_executions = self.compiled_executions.saturating_add(1);
        self.compiled_execution_instrs = self.compiled_execution_instrs.saturating_add(instr_count);
    }

    pub fn compiled_block_count(&self) -> usize {
        self.compiled_blocks.len()
    }

    pub fn stats(&self) -> RuntimeWasmStats {
        RuntimeWasmStats {
            compiled_executions: self.compiled_executions,
            compiled_execution_instrs: self.compiled_execution_instrs,
            compile_attempts: self.compile_attempts,
            compiled_blocks: self.compiled_blocks.len(),
            unprofitable_fallbacks: self.unprofitable_fallbacks,
            unsupported_fallbacks: self.unsupported_fallbacks,
            #[cfg(target_arch = "wasm32")]
            shared_rebuilds: self.shared.rebuilds,
            #[cfg(not(target_arch = "wasm32"))]
            shared_rebuilds: 0,
            #[cfg(target_arch = "wasm32")]
            shared_dispatch_blocks: self.shared.dispatch_blocks,
            #[cfg(not(target_arch = "wasm32"))]
            shared_dispatch_blocks: 0,
        }
    }

    pub fn current_slice(&self) -> (u64, u64) {
        (self.begin_cycle, self.begin_deadline)
    }

    pub fn top_fallback_reasons(&self, limit: usize) -> Vec<(UnsupportedOpcode, u64)> {
        let mut reasons: Vec<_> = self
            .fallback_reason_counts
            .iter()
            .map(|(reason, count)| (*reason, *count))
            .collect();
        reasons.sort_unstable_by(|(left_reason, left_count), (right_reason, right_count)| {
            right_count
                .cmp(left_count)
                .then_with(|| fallback_reason_sort_key(*left_reason).cmp(&fallback_reason_sort_key(*right_reason)))
        });
        reasons.truncate(limit);
        reasons
    }

    pub fn block_spec(&self, fingerprint: &BlockFingerprint) -> Option<&BlockSpec> {
        self.block_specs.get(fingerprint)
    }

    pub fn compiled_fingerprint_for_pc(&self, pc: u32) -> Option<BlockFingerprint> {
        self.pc_index.get(&pc).copied()
    }

    pub fn drive_hot_candidate(
        &mut self,
        sys: &crate::system::System<SYSTEM>,
    ) -> Option<BlockCompileDecision> {
        let pc = self.poll_hot_candidate()?;
        let decision = self.compile_candidate(sys, pc);
        if let BlockCompileDecision::Fallback { pc, reason } = &decision {
            self.note_fallback(*pc, *reason);
        }
        Some(decision)
    }

    fn block_unsupported_reason(&self, spec: &BlockSpec) -> Option<UnsupportedOpcode> {
        if spec.instrs.is_empty() {
            return Some(UnsupportedOpcode::Terminator(TermKind::LengthCap));
        }

        if spec.len() < MIN_COMPILED_BLOCK_INSTRS {
            return Some(UnsupportedOpcode::Unprofitable {
                instr_count: spec.len() as u16,
            });
        }

        let last_idx = spec.instrs.len() - 1;
        for (idx, raw) in spec.instrs.iter().enumerate() {
            let instr = Instruction(*raw);
            let is_last = idx == last_idx;
            if is_last {
                if let Some(reason) = self.terminator_unsupported_reason(spec, instr) {
                    return Some(reason);
                }
                continue;
            }

            if let Some(reason) = self.instruction_unsupported_reason(instr) {
                return Some(reason);
            }
        }

        None
    }

    fn unique_trace_blocks(&self) -> Vec<BlockSpec> {
        let mut blocks = BTreeMap::new();
        for compiled in self.compiled_blocks.values() {
            for block in compiled.trace.all_blocks() {
                blocks.entry(block.start_pc).or_insert_with(|| block.clone());
            }
        }
        blocks.into_values().collect()
    }

    #[cfg(target_arch = "wasm32")]
    fn ensure_shared_runner(&mut self) -> Option<&JsValue> {
        if self.shared.read_u32_import.is_none() {
            self.shared.read_u32_import = Some(make_runtime_wasm_read_u32_import::<SYSTEM>());
        }
        if self.shared.read_u16_import.is_none() {
            self.shared.read_u16_import = Some(make_runtime_wasm_read_u16_import::<SYSTEM>());
        }
        if self.shared.read_u8_import.is_none() {
            self.shared.read_u8_import = Some(make_runtime_wasm_read_u8_import::<SYSTEM>());
        }
        if self.shared.write_u32_import.is_none() {
            self.shared.write_u32_import = Some(make_runtime_wasm_write_u32_import::<SYSTEM>());
        }
        if self.shared.write_u16_import.is_none() {
            self.shared.write_u16_import = Some(make_runtime_wasm_write_u16_import::<SYSTEM>());
        }
        if self.shared.write_u8_import.is_none() {
            self.shared.write_u8_import = Some(make_runtime_wasm_write_u8_import::<SYSTEM>());
        }
        if self.shared.imported_memory.is_none() {
            self.shared.imported_memory = Some(wasm_bindgen::memory());
        }

        if self.shared.runner.is_none() || self.shared.dirty {
            if self.compiled_blocks.is_empty() {
                self.shared.dispatch_blocks = 0;
                return None;
            }

            let bytes = self.build_shared_wasm_module();
            let read_u32 = self.shared.read_u32_import.as_ref().expect("read_u32 import should exist");
            let read_u16 = self.shared.read_u16_import.as_ref().expect("read_u16 import should exist");
            let read_u8 = self.shared.read_u8_import.as_ref().expect("read_u8 import should exist");
            let write_u32 = self.shared.write_u32_import.as_ref().expect("write_u32 import should exist");
            let write_u16 = self.shared.write_u16_import.as_ref().expect("write_u16 import should exist");
            let write_u8 = self.shared.write_u8_import.as_ref().expect("write_u8 import should exist");
            let memory = self.shared.imported_memory.as_ref().expect("imported memory should exist");
            let state_ptr = self.shared.state_storage.as_ptr() as usize as u32;
            match gecko_runtime_wasm_compile(
                &bytes,
                read_u32.as_ref(),
                read_u16.as_ref(),
                read_u8.as_ref(),
                write_u32.as_ref(),
                write_u16.as_ref(),
                write_u8.as_ref(),
                memory,
                state_ptr,
            ) {
                Ok(run) => {
                    self.shared.runner = Some(run);
                    self.shared.dirty = false;
                    self.shared.rebuilds = self.shared.rebuilds.saturating_add(1);
                    self.shared.dispatch_blocks = self.unique_trace_blocks().len();
                }
                Err(err) => {
                    tracing::warn!(?err, "shared runtime wasm compile failed");
                    self.shared.runner = None;
                    return None;
                }
            }
        }

        self.shared.runner.as_ref()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn execute_compiled(
        &mut self,
        sys: *mut crate::system::System<SYSTEM>,
        entry_pc: u32,
        gprs: &[u32; 32],
        cr: u32,
        ctr: u32,
        lr: u32,
        so: u32,
        ov: u32,
        ca: u32,
    ) -> Option<([u32; 32], u32, u32, u32, u32, u32, u32, u32, u32, u64)> {
        self.compiled_fingerprint_for_pc(entry_pc)?;
        self.shared.mem1_base = unsafe { (*sys).mmio.ram_ptr as u32 };
        self.shared.code_refcount_base = unsafe { (*sys).mmio.code_refcount_ptr as u32 };
        let runner = self.ensure_shared_runner()?.clone();

        let mut state = [0u32; STATE_WORDS];
        state[..32].copy_from_slice(&gprs[..]);
        state[CR_PARAM_INDEX as usize] = cr;
        state[CTR_PARAM_INDEX as usize] = ctr;
        state[SO_PARAM_INDEX as usize] = so & 1;
        state[OV_PARAM_INDEX as usize] = ov & 1;
        state[CA_PARAM_INDEX as usize] = ca & 1;
        state[PC_STATE_INDEX as usize] = entry_pc;
        state[LAST_PC_STATE_INDEX as usize] = entry_pc;
        state[LR_STATE_INDEX as usize] = lr;

        let result = with_active_runtime_wasm_system(sys, || gecko_runtime_wasm_invoke(&runner, &state));
        if result.length() < STATE_WORDS as u32 {
            tracing::warn!(results = result.length(), "shared runtime wasm returned incomplete register state");
            return None;
        }

        let mut next_gprs = [0u32; 32];
        for (idx, slot) in next_gprs.iter_mut().enumerate() {
            *slot = result.get_index(idx as u32);
        }
        let next_cr = result.get_index(CR_PARAM_INDEX);
        let next_ctr = result.get_index(CTR_PARAM_INDEX);
        let next_so = result.get_index(SO_PARAM_INDEX) & 1;
        let next_ov = result.get_index(OV_PARAM_INDEX) & 1;
        let next_ca = result.get_index(CA_PARAM_INDEX) & 1;
        let next_pc = result.get_index(PC_STATE_INDEX);
        let last_pc = result.get_index(LAST_PC_STATE_INDEX);
        let next_lr = result.get_index(LR_STATE_INDEX);
        let executed_instrs = u64::from(result.get_index(EXECUTED_INSTRS_STATE_INDEX));

        if (next_pc & 0x3) != 0 {
            tracing::warn!(next_pc = format!("{:08X}", next_pc), "shared runtime wasm returned misaligned next_pc");
            return None;
        }

        Some((next_gprs, next_cr, next_ctr, next_pc, last_pc, next_lr, next_so, next_ov, next_ca, executed_instrs))
    }

    fn instruction_unsupported_reason(&self, instr: Instruction) -> Option<UnsupportedOpcode> {
        // Keep this whitelist limited to opcodes whose lowering semantics have
        // been checked against the core emulator behavior. Parsing as wasm is
        // not a sufficient bar for adding a new opcode here.
        match primary_opcode(instr) {
            10 | 11 | 13 | 12 | 14 | 15 | 21 | 24 | 25 | 26 | 27 | 28 | 29 | 32 | 33 | 34 | 35 | 36 | 37 | 38 | 39 | 40
            | 41 | 42 | 43 | 44 | 45 => None,
            31 => match xo10(instr) {
                0 | 8 | 10 | 23 | 24 | 28 | 32 | 40 | 55 | 87 | 119 | 151 | 183 | 215 | 247 | 266 | 279 | 311
                | 316 | 407 | 439 | 444 | 536 | 792 | 824 => None,
                xo => Some(UnsupportedOpcode::Xo10(xo)),
            },
            other => Some(UnsupportedOpcode::Primary(other)),
        }
    }

    fn terminator_unsupported_reason(
        &self,
        spec: &BlockSpec,
        instr: Instruction,
    ) -> Option<UnsupportedOpcode> {
        match spec.terminator {
            TermKind::LengthCap => None,
            TermKind::Branch => {
                if primary_opcode(instr) != 18 || instr.lk() {
                    Some(UnsupportedOpcode::Terminator(TermKind::Branch))
                } else {
                    None
                }
            }
            TermKind::BranchLink => {
                if primary_opcode(instr) != 18 || !instr.lk() {
                    Some(UnsupportedOpcode::Terminator(TermKind::BranchLink))
                } else {
                    None
                }
            }
            TermKind::BranchCond => {
                if primary_opcode(instr) != 16 || instr.lk() {
                    Some(UnsupportedOpcode::Terminator(TermKind::BranchCond))
                } else {
                    None
                }
            }
            TermKind::BranchToReg => {
                if primary_opcode(instr) != 19 || !matches!(xo10(instr), 16 | 528) {
                    Some(UnsupportedOpcode::Terminator(TermKind::BranchToReg))
                } else {
                    None
                }
            }
            other => Some(UnsupportedOpcode::Terminator(other)),
        }
    }

    fn build_wasm_trace_module(&self, fingerprint: &BlockFingerprint, trace: &TraceSpec) -> Vec<u8> {
        let blocks: Vec<_> = trace.all_blocks().cloned().collect();
        self.build_wasm_dispatch_module(&blocks, Some(fingerprint))
    }

    fn build_shared_wasm_module(&self) -> Vec<u8> {
        let blocks = self.unique_trace_blocks();
        self.build_wasm_dispatch_module(&blocks, None)
    }

    fn build_wasm_dispatch_module(&self, blocks: &[BlockSpec], fingerprint: Option<&BlockFingerprint>) -> Vec<u8> {
        let state_ptr = self.runtime_state_ptr();
        let mut buckets: Vec<Vec<&BlockSpec>> = (0..DISPATCH_BUCKETS).map(|_| Vec::new()).collect();
        for block in blocks {
            buckets[((block.start_pc >> 2) & DISPATCH_BUCKET_MASK) as usize].push(block);
        }
        let mut module = Vec::new();
        module.extend_from_slice(b"\0asm");
        module.extend_from_slice(&1u32.to_le_bytes());

        let mut type_section = Vec::new();
        write_uleb(&mut type_section, 3);
        type_section.push(0x60);
        write_uleb(&mut type_section, 1);
        type_section.push(0x7f);
        write_uleb(&mut type_section, 1);
        type_section.push(0x7f);
        type_section.push(0x60);
        write_uleb(&mut type_section, 2);
        type_section.push(0x7f);
        type_section.push(0x7f);
        write_uleb(&mut type_section, 0);
        type_section.push(0x60);
        write_uleb(&mut type_section, 0);
        write_uleb(&mut type_section, 0);
        push_section(&mut module, 1, &type_section);

        let mut import_section = Vec::new();
        write_uleb(&mut import_section, 7);
        write_uleb(&mut import_section, 3);
        import_section.extend_from_slice(b"env");
        write_uleb(&mut import_section, 8);
        import_section.extend_from_slice(b"read_u32");
        import_section.push(0x00);
        write_uleb(&mut import_section, READ_SCALAR_FUNCTION_TYPE_INDEX);
        write_uleb(&mut import_section, 3);
        import_section.extend_from_slice(b"env");
        write_uleb(&mut import_section, 8);
        import_section.extend_from_slice(b"read_u16");
        import_section.push(0x00);
        write_uleb(&mut import_section, READ_SCALAR_FUNCTION_TYPE_INDEX);
        write_uleb(&mut import_section, 3);
        import_section.extend_from_slice(b"env");
        write_uleb(&mut import_section, 7);
        import_section.extend_from_slice(b"read_u8");
        import_section.push(0x00);
        write_uleb(&mut import_section, READ_SCALAR_FUNCTION_TYPE_INDEX);
        write_uleb(&mut import_section, 3);
        import_section.extend_from_slice(b"env");
        write_uleb(&mut import_section, 9);
        import_section.extend_from_slice(b"write_u32");
        import_section.push(0x00);
        write_uleb(&mut import_section, WRITE_SCALAR_FUNCTION_TYPE_INDEX);
        write_uleb(&mut import_section, 3);
        import_section.extend_from_slice(b"env");
        write_uleb(&mut import_section, 9);
        import_section.extend_from_slice(b"write_u16");
        import_section.push(0x00);
        write_uleb(&mut import_section, WRITE_SCALAR_FUNCTION_TYPE_INDEX);
        write_uleb(&mut import_section, 3);
        import_section.extend_from_slice(b"env");
        write_uleb(&mut import_section, 8);
        import_section.extend_from_slice(b"write_u8");
        import_section.push(0x00);
        write_uleb(&mut import_section, WRITE_SCALAR_FUNCTION_TYPE_INDEX);
        write_uleb(&mut import_section, 3);
        import_section.extend_from_slice(b"env");
        write_uleb(&mut import_section, 6);
        import_section.extend_from_slice(b"memory");
        import_section.push(0x02);
        import_section.push(0x00);
        write_uleb(&mut import_section, 1);
        push_section(&mut module, 2, &import_section);

        let mut func_section = Vec::new();
        write_uleb(&mut func_section, 1);
        write_uleb(&mut func_section, RUN_FUNCTION_TYPE_INDEX);
        push_section(&mut module, 3, &func_section);

        let mut export_section = Vec::new();
        write_uleb(&mut export_section, 1);
        write_uleb(&mut export_section, 3);
        export_section.extend_from_slice(b"run");
        export_section.push(0x00);
        write_uleb(&mut export_section, RUN_FUNCTION_INDEX);
        push_section(&mut module, 7, &export_section);

        let mut code_section = Vec::new();
        write_uleb(&mut code_section, 1);

        let mut body = Vec::new();
        write_uleb(&mut body, 1); // local decl count
        write_uleb(&mut body, 44);
        body.push(0x7f); // state locals + next_pc + last_pc + cmp lhs + cmp rhs locals

        for reg in 0..GPR_PARAM_COUNT {
            emit_state_load_local(&mut body, state_ptr, reg, reg);
        }
        emit_state_load_local(&mut body, state_ptr, CR_PARAM_INDEX, CR_PARAM_INDEX);
        emit_state_load_local(&mut body, state_ptr, CTR_PARAM_INDEX, CTR_PARAM_INDEX);
        emit_state_load_local(&mut body, state_ptr, SO_PARAM_INDEX, SO_PARAM_INDEX);
        emit_state_load_local(&mut body, state_ptr, OV_PARAM_INDEX, OV_PARAM_INDEX);
        emit_state_load_local(&mut body, state_ptr, CA_PARAM_INDEX, CA_PARAM_INDEX);
        emit_state_load_local(&mut body, state_ptr, PC_STATE_INDEX, CURRENT_PC_LOCAL_INDEX);
        emit_state_load_local(&mut body, state_ptr, LAST_PC_STATE_INDEX, LAST_PC_LOCAL_INDEX);
        emit_state_load_local(&mut body, state_ptr, LR_STATE_INDEX, LR_LOCAL_INDEX);
        emit_i32_const(&mut body, 0);
        emit_local_set(&mut body, EXECUTED_INSTRS_LOCAL_INDEX);

        body.push(0x02); // block
        body.push(0x40);
        body.push(0x03); // loop
        body.push(0x40);

        emit_local_get(&mut body, CURRENT_PC_LOCAL_INDEX);
        emit_i32_const(&mut body, 2);
        body.push(0x76); // i32.shr_u
        emit_i32_const(&mut body, DISPATCH_BUCKET_MASK as i32);
        body.push(0x71); // i32.and
        emit_local_set(&mut body, CMP_LHS_LOCAL_INDEX);

        for (bucket_index, bucket_blocks) in buckets.iter().enumerate() {
            if bucket_blocks.is_empty() {
                continue;
            }

            emit_local_get(&mut body, CMP_LHS_LOCAL_INDEX);
            emit_i32_const(&mut body, bucket_index as i32);
            body.push(0x46); // i32.eq
            body.push(0x04); // if
            body.push(0x40);

            for block in bucket_blocks {
                emit_local_get(&mut body, CURRENT_PC_LOCAL_INDEX);
                emit_i32_const(&mut body, block.start_pc as i32);
                body.push(0x46); // i32.eq
                body.push(0x04); // if
                body.push(0x40);

                emit_i32_const(&mut body, block.end_pc() as i32);
                emit_local_set(&mut body, NEXT_PC_LOCAL_INDEX);

                for (idx, raw) in block.instrs.iter().enumerate() {
                    let instr = Instruction(*raw);
                    if idx == block.instrs.len() - 1 {
                        let stub_fingerprint = fingerprint.copied().unwrap_or(BlockFingerprint {
                            pc: block.start_pc,
                            instr_count: block.len() as u16,
                            hash: 0,
                        });
                        self.emit_lowered_terminator(
                            &mut body,
                            block,
                            &stub_fingerprint,
                            instr,
                            block.pcs[idx],
                        );
                    } else {
                        self.emit_lowered_integer_op(&mut body, instr);
                    }
                }

                emit_i32_const(&mut body, block.pcs.last().copied().unwrap_or(block.start_pc) as i32);
                emit_local_set(&mut body, LAST_PC_LOCAL_INDEX);
                emit_local_get(&mut body, EXECUTED_INSTRS_LOCAL_INDEX);
                emit_i32_const(&mut body, block.len() as i32);
                body.push(0x6a); // i32.add
                emit_local_set(&mut body, EXECUTED_INSTRS_LOCAL_INDEX);
                emit_local_get(&mut body, NEXT_PC_LOCAL_INDEX);
                emit_local_set(&mut body, CURRENT_PC_LOCAL_INDEX);
                body.push(0x0c); // br 2 => loop head
                write_uleb(&mut body, 2);
                body.push(0x0b); // end if
            }

            body.push(0x0b); // end bucket if
        }

        body.push(0x0c); // br 1 => exit when current_pc leaves compiled region
        write_uleb(&mut body, 1);
        body.push(0x0b); // end loop
        body.push(0x0b); // end block

        for reg in 0..GPR_PARAM_COUNT {
            emit_state_store_local(&mut body, state_ptr, reg, reg);
        }
        emit_state_store_local(&mut body, state_ptr, CR_PARAM_INDEX, CR_PARAM_INDEX);
        emit_state_store_local(&mut body, state_ptr, CTR_PARAM_INDEX, CTR_PARAM_INDEX);
        emit_state_store_local(&mut body, state_ptr, SO_PARAM_INDEX, SO_PARAM_INDEX);
        emit_state_store_local(&mut body, state_ptr, OV_PARAM_INDEX, OV_PARAM_INDEX);
        emit_state_store_local(&mut body, state_ptr, CA_PARAM_INDEX, CA_PARAM_INDEX);
        emit_state_store_local(&mut body, state_ptr, PC_STATE_INDEX, CURRENT_PC_LOCAL_INDEX);
        emit_state_store_local(&mut body, state_ptr, LAST_PC_STATE_INDEX, LAST_PC_LOCAL_INDEX);
        emit_state_store_local(&mut body, state_ptr, LR_STATE_INDEX, LR_LOCAL_INDEX);
        emit_state_store_local(&mut body, state_ptr, EXECUTED_INSTRS_STATE_INDEX, EXECUTED_INSTRS_LOCAL_INDEX);
        body.push(0x0b); // end

        write_uleb(&mut code_section, body.len() as u32);
        code_section.extend_from_slice(&body);
        push_section(&mut module, 10, &code_section);

        // Custom section to help with debugging/caching.
        let mut custom = Vec::new();
        write_uleb(&mut custom, 10);
        custom.extend_from_slice(b"gecko-wasm");
        if let Some(fingerprint) = fingerprint {
            write_uleb(&mut custom, fingerprint.pc);
            write_uleb(&mut custom, fingerprint.instr_count as u32);
            write_uleb(&mut custom, fingerprint.hash as u32);
            write_uleb(&mut custom, (fingerprint.hash >> 32) as u32);
        } else {
            write_uleb(&mut custom, 0);
            write_uleb(&mut custom, blocks.len() as u32);
            write_uleb(&mut custom, 0);
            write_uleb(&mut custom, 0);
        }
        push_section(&mut module, 0, &custom);

        module
    }

    #[cfg(test)]
    fn build_wasm_module(&self, fingerprint: &BlockFingerprint, spec: &BlockSpec) -> Vec<u8> {
        self.build_wasm_trace_module(
            fingerprint,
            &TraceSpec {
                entry: spec.clone(),
                successors: Vec::new(),
            },
        )
    }

    fn emit_lowered_integer_op(&self, out: &mut Vec<u8>, instr: Instruction) {
        match primary_opcode(instr) {
            10 => self.emit_cmp_imm(out, instr, false),
            11 => self.emit_cmp_imm(out, instr, true),
            12 => self.emit_subfic(out, instr),
            13 => self.emit_addic(out, instr, true),
            14 => self.emit_addi_like(out, instr, false),
            15 => self.emit_addi_like(out, instr, true),
            21 => self.emit_rlwinm(out, instr),
            24 => self.emit_logical_imm(out, instr, 0x72, false),
            25 => self.emit_logical_imm(out, instr, 0x72, true),
            26 => self.emit_logical_imm(out, instr, 0x73, false),
            27 => self.emit_logical_imm(out, instr, 0x73, true),
            28 => self.emit_andi_dot_like(out, instr, false),
            29 => self.emit_andi_dot_like(out, instr, true),
            32 => self.emit_lwz(out, instr),
            33 => self.emit_lwzu(out, instr),
            34 => self.emit_lbz(out, instr),
            35 => self.emit_lbzu(out, instr),
            36 => self.emit_stw(out, instr),
            37 => self.emit_stwu(out, instr),
            38 => self.emit_stb(out, instr),
            39 => self.emit_stbu(out, instr),
            40 => self.emit_lhz(out, instr),
            41 => self.emit_lhzu(out, instr),
            42 => self.emit_lha(out, instr),
            43 => self.emit_lhau(out, instr),
            44 => self.emit_sth(out, instr),
            45 => self.emit_sthu(out, instr),
            31 => match xo10(instr) {
                0 => self.emit_cmp_xform(out, instr, true),
                8 => self.emit_subfcx(out, instr),
                10 => self.emit_addcx(out, instr),
                23 => self.emit_lwzx(out, instr),
                24 => self.emit_slwx(out, instr),
                28 => self.emit_andx(out, instr),
                32 => self.emit_cmp_xform(out, instr, false),
                40 => self.emit_subfx(out, instr),
                55 => self.emit_lwzux(out, instr),
                87 => self.emit_lbzx(out, instr),
                119 => self.emit_lbzux(out, instr),
                151 => self.emit_stwx(out, instr),
                183 => self.emit_stwux(out, instr),
                215 => self.emit_stbx(out, instr),
                247 => self.emit_stbux(out, instr),
                266 => self.emit_addx(out, instr),
                279 => self.emit_lhzx(out, instr),
                311 => self.emit_lhzux(out, instr),
                316 => self.emit_xorx(out, instr),
                407 => self.emit_sthx(out, instr),
                439 => self.emit_sthux(out, instr),
                444 => self.emit_orx(out, instr),
                536 => self.emit_srwx(out, instr),
                792 => self.emit_srawx(out, instr),
                824 => self.emit_srawix(out, instr),
                xo => panic!("attempted to lower unsupported xo10 opcode {xo:#x}"),
            },
            other => panic!("attempted to lower unsupported opcode {other:#x}"),
        }
    }

    fn emit_lowered_terminator(
        &self,
        out: &mut Vec<u8>,
        spec: &BlockSpec,
        _fingerprint: &BlockFingerprint,
        instr: Instruction,
        branch_pc: u32,
    ) {
        match spec.terminator {
            TermKind::LengthCap => {}
            TermKind::Branch => {
                let target = if instr.aa() {
                    instr.li() as u32
                } else {
                    branch_pc.wrapping_add_signed(instr.li())
                };
                emit_i32_const(out, target as i32);
                emit_local_set(out, NEXT_PC_LOCAL_INDEX);
            }
            TermKind::BranchLink => {
                emit_i32_const(out, branch_pc.wrapping_add(4) as i32);
                emit_local_set(out, LR_LOCAL_INDEX);

                let target = if instr.aa() {
                    instr.li() as u32
                } else {
                    branch_pc.wrapping_add_signed(instr.li())
                };
                emit_i32_const(out, target as i32);
                emit_local_set(out, NEXT_PC_LOCAL_INDEX);
            }
            TermKind::BranchCond => {
                self.emit_bc_terminator(out, instr, branch_pc);
            }
            TermKind::BranchToReg => {
                self.emit_branch_to_reg_terminator(out, instr, branch_pc);
            }
            _ => unreachable!("unsupported terminator reached lowering"),
        }
    }

    fn emit_addi_like(&self, out: &mut Vec<u8>, instr: Instruction, addis: bool) {
        let src = instr.ra();
        if src == 0 {
            emit_i32_const(out, 0);
        } else {
            emit_local_get(out, src as u32);
        }

        let imm = if addis {
            (instr.simm() << 16) as i32
        } else {
            instr.simm()
        };
        emit_i32_const(out, imm);
        out.push(0x6a); // i32.add
        emit_local_set(out, instr.rd() as u32);
    }

    fn emit_subfic(&self, out: &mut Vec<u8>, instr: Instruction) {
        emit_i32_const(out, instr.simm());
        emit_local_get(out, instr.ra() as u32);
        out.push(0x6b); // i32.sub
        emit_local_set(out, instr.rd() as u32);

        emit_i32_const(out, instr.simm());
        emit_local_get(out, instr.ra() as u32);
        out.push(0x4f); // i32.ge_u
        emit_local_set(out, CA_PARAM_INDEX);
    }

    fn emit_addic(&self, out: &mut Vec<u8>, instr: Instruction, rc: bool) {
        emit_local_get(out, instr.ra() as u32);
        emit_i32_const(out, instr.simm());
        out.push(0x6a); // i32.add
        emit_local_set(out, instr.rd() as u32);

        emit_local_get(out, instr.rd() as u32);
        emit_local_get(out, instr.ra() as u32);
        out.push(0x49); // i32.lt_u
        emit_local_set(out, CA_PARAM_INDEX);

        if rc {
            self.emit_update_cr0_from_reg(out, instr.rd() as u32);
        }
    }

    fn emit_update_ov_so_from_add_like(&self, out: &mut Vec<u8>, lhs_local: u32, rhs_local: u32, res_local: u32) {
        // OV = (((lhs ^ res) & (rhs ^ res)) >> 31) != 0
        emit_local_get(out, lhs_local);
        emit_local_get(out, res_local);
        out.push(0x73); // i32.xor
        emit_local_get(out, rhs_local);
        emit_local_get(out, res_local);
        out.push(0x73); // i32.xor
        out.push(0x71); // i32.and
        emit_i32_const(out, 31);
        out.push(0x76); // i32.shr_u
        emit_local_set(out, OV_PARAM_INDEX);

        emit_local_get(out, SO_PARAM_INDEX);
        emit_local_get(out, OV_PARAM_INDEX);
        out.push(0x72); // i32.or
        emit_local_set(out, SO_PARAM_INDEX);
    }

    fn emit_addx(&self, out: &mut Vec<u8>, instr: Instruction) {
        emit_local_get(out, instr.ra() as u32);
        emit_local_get(out, instr.rb() as u32);
        out.push(0x6a); // i32.add
        emit_local_set(out, instr.rd() as u32);

        if instr.oe() {
            self.emit_update_ov_so_from_add_like(out, instr.ra() as u32, instr.rb() as u32, instr.rd() as u32);
        }
        if instr.rc() {
            self.emit_update_cr0_from_reg(out, instr.rd() as u32);
        }
    }

    fn emit_subfx(&self, out: &mut Vec<u8>, instr: Instruction) {
        emit_local_get(out, instr.rb() as u32);
        emit_local_get(out, instr.ra() as u32);
        out.push(0x6b); // i32.sub
        emit_local_set(out, instr.rd() as u32);

        if instr.oe() {
            emit_local_get(out, instr.ra() as u32);
            emit_i32_const(out, -1);
            out.push(0x73); // i32.xor
            emit_local_set(out, CMP_LHS_LOCAL_INDEX);
            self.emit_update_ov_so_from_add_like(out, CMP_LHS_LOCAL_INDEX, instr.rb() as u32, instr.rd() as u32);
        }
        if instr.rc() {
            self.emit_update_cr0_from_reg(out, instr.rd() as u32);
        }
    }

    fn emit_addcx(&self, out: &mut Vec<u8>, instr: Instruction) {
        emit_local_get(out, instr.ra() as u32);
        emit_local_get(out, instr.rb() as u32);
        out.push(0x6a); // i32.add
        emit_local_set(out, instr.rd() as u32);

        emit_local_get(out, instr.rd() as u32);
        emit_local_get(out, instr.ra() as u32);
        out.push(0x49); // i32.lt_u
        emit_local_set(out, CA_PARAM_INDEX);

        if instr.oe() {
            self.emit_update_ov_so_from_add_like(out, instr.ra() as u32, instr.rb() as u32, instr.rd() as u32);
        }
        if instr.rc() {
            self.emit_update_cr0_from_reg(out, instr.rd() as u32);
        }
    }

    fn emit_subfcx(&self, out: &mut Vec<u8>, instr: Instruction) {
        emit_local_get(out, instr.rb() as u32);
        emit_local_get(out, instr.ra() as u32);
        out.push(0x6b); // i32.sub
        emit_local_set(out, instr.rd() as u32);

        emit_local_get(out, instr.rb() as u32);
        emit_local_get(out, instr.ra() as u32);
        out.push(0x4f); // i32.ge_u
        emit_local_set(out, CA_PARAM_INDEX);

        if instr.oe() {
            emit_local_get(out, instr.ra() as u32);
            emit_i32_const(out, -1);
            out.push(0x73); // i32.xor
            emit_local_set(out, CMP_LHS_LOCAL_INDEX);
            self.emit_update_ov_so_from_add_like(out, CMP_LHS_LOCAL_INDEX, instr.rb() as u32, instr.rd() as u32);
        }
        if instr.rc() {
            self.emit_update_cr0_from_reg(out, instr.rd() as u32);
        }
    }

    fn emit_slwx(&self, out: &mut Vec<u8>, instr: Instruction) {
        emit_local_get(out, instr.rb() as u32);
        emit_i32_const(out, 0x3f);
        out.push(0x71); // i32.and
        emit_local_set(out, CMP_LHS_LOCAL_INDEX);

        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_i32_const(out, 32);
        out.push(0x4f); // i32.ge_u
        out.push(0x04); // if
        out.push(0x40);
        emit_i32_const(out, 0);
        emit_local_set(out, instr.ra() as u32);
        out.push(0x05); // else
        out.push(0x40);
        emit_local_get(out, instr.rs() as u32);
        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        out.push(0x74); // i32.shl
        emit_local_set(out, instr.ra() as u32);
        out.push(0x0b); // end if

        if instr.rc() {
            self.emit_update_cr0_from_reg(out, instr.ra() as u32);
        }
    }

    fn emit_srwx(&self, out: &mut Vec<u8>, instr: Instruction) {
        emit_local_get(out, instr.rb() as u32);
        emit_i32_const(out, 0x3f);
        out.push(0x71); // i32.and
        emit_local_set(out, CMP_LHS_LOCAL_INDEX);

        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_i32_const(out, 32);
        out.push(0x4f); // i32.ge_u
        out.push(0x04); // if
        out.push(0x40);
        emit_i32_const(out, 0);
        emit_local_set(out, instr.ra() as u32);
        out.push(0x05); // else
        out.push(0x40);
        emit_local_get(out, instr.rs() as u32);
        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        out.push(0x76); // i32.shr_u
        emit_local_set(out, instr.ra() as u32);
        out.push(0x0b); // end if

        if instr.rc() {
            self.emit_update_cr0_from_reg(out, instr.ra() as u32);
        }
    }

    fn emit_srawx(&self, out: &mut Vec<u8>, instr: Instruction) {
        emit_local_get(out, instr.rb() as u32);
        emit_i32_const(out, 0x3f);
        out.push(0x71); // i32.and
        emit_local_set(out, CMP_LHS_LOCAL_INDEX);

        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_i32_const(out, 32);
        out.push(0x4f); // i32.ge_u
        out.push(0x04); // if
        out.push(0x40);

        emit_local_get(out, instr.rs() as u32);
        emit_i32_const(out, 0);
        out.push(0x48); // i32.lt_s
        emit_local_set(out, CA_PARAM_INDEX);
        emit_local_get(out, instr.rs() as u32);
        emit_i32_const(out, 31);
        out.push(0x75); // i32.shr_s
        emit_local_set(out, instr.ra() as u32);

        out.push(0x05); // else
        out.push(0x40);

        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_i32_const(out, 0);
        out.push(0x46); // i32.eq
        out.push(0x04); // if
        out.push(0x40);

        emit_i32_const(out, 0);
        emit_local_set(out, CA_PARAM_INDEX);
        emit_local_get(out, instr.rs() as u32);
        emit_local_set(out, instr.ra() as u32);

        out.push(0x05); // else
        out.push(0x40);

        emit_i32_const(out, 1);
        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        out.push(0x74); // i32.shl
        emit_i32_const(out, 1);
        out.push(0x6b); // i32.sub
        emit_local_set(out, CMP_RHS_LOCAL_INDEX);

        emit_local_get(out, instr.rs() as u32);
        emit_i32_const(out, 0);
        out.push(0x48); // i32.lt_s
        emit_local_get(out, instr.rs() as u32);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        out.push(0x71); // i32.and
        emit_i32_const(out, 0);
        out.push(0x47); // i32.ne
        out.push(0x71); // i32.and
        emit_local_set(out, CA_PARAM_INDEX);

        emit_local_get(out, instr.rs() as u32);
        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        out.push(0x75); // i32.shr_s
        emit_local_set(out, instr.ra() as u32);

        out.push(0x0b); // end inner if
        out.push(0x0b); // end outer else
        out.push(0x0b); // end if

        if instr.rc() {
            self.emit_update_cr0_from_reg(out, instr.ra() as u32);
        }
    }

    fn emit_srawix(&self, out: &mut Vec<u8>, instr: Instruction) {
        let sh = instr.sh() as u32;
        if sh == 0 {
            emit_i32_const(out, 0);
            emit_local_set(out, CA_PARAM_INDEX);
            emit_local_get(out, instr.rs() as u32);
            emit_local_set(out, instr.ra() as u32);
        } else {
            emit_i32_const(out, (1u32 << sh) as i32);
            emit_i32_const(out, 1);
            out.push(0x6b); // i32.sub
            emit_local_set(out, CMP_RHS_LOCAL_INDEX);

            emit_local_get(out, instr.rs() as u32);
            emit_i32_const(out, 0);
            out.push(0x48); // i32.lt_s
            emit_local_get(out, instr.rs() as u32);
            emit_local_get(out, CMP_RHS_LOCAL_INDEX);
            out.push(0x71); // i32.and
            emit_i32_const(out, 0);
            out.push(0x47); // i32.ne
            out.push(0x71); // i32.and
            emit_local_set(out, CA_PARAM_INDEX);

            emit_local_get(out, instr.rs() as u32);
            emit_i32_const(out, sh as i32);
            out.push(0x75); // i32.shr_s
            emit_local_set(out, instr.ra() as u32);
        }

        if instr.rc() {
            self.emit_update_cr0_from_reg(out, instr.ra() as u32);
        }
    }

    fn emit_rlwinm(&self, out: &mut Vec<u8>, instr: Instruction) {
        emit_local_get(out, instr.rs() as u32);
        emit_i32_const(out, instr.sh() as i32);
        out.push(0x77); // i32.rotl
        emit_i32_const(out, rlwinm_mask(instr.mb() as u32, instr.me() as u32) as i32);
        out.push(0x71); // i32.and
        emit_local_set(out, instr.ra() as u32);

        if instr.rc() {
            self.emit_update_cr0_from_reg(out, instr.ra() as u32);
        }
    }

    fn emit_orx(&self, out: &mut Vec<u8>, instr: Instruction) {
        emit_local_get(out, instr.rs() as u32);
        emit_local_get(out, instr.rb() as u32);
        out.push(0x72); // i32.or
        emit_local_set(out, instr.ra() as u32);

        if instr.rc() {
            self.emit_update_cr0_from_reg(out, instr.ra() as u32);
        }
    }

    fn emit_andx(&self, out: &mut Vec<u8>, instr: Instruction) {
        emit_local_get(out, instr.rs() as u32);
        emit_local_get(out, instr.rb() as u32);
        out.push(0x71); // i32.and
        emit_local_set(out, instr.ra() as u32);

        if instr.rc() {
            self.emit_update_cr0_from_reg(out, instr.ra() as u32);
        }
    }

    fn emit_xorx(&self, out: &mut Vec<u8>, instr: Instruction) {
        emit_local_get(out, instr.rs() as u32);
        emit_local_get(out, instr.rb() as u32);
        out.push(0x73); // i32.xor
        emit_local_set(out, instr.ra() as u32);

        if instr.rc() {
            self.emit_update_cr0_from_reg(out, instr.ra() as u32);
        }
    }

    fn emit_logical_imm(&self, out: &mut Vec<u8>, instr: Instruction, op: u8, upper: bool) {
        emit_local_get(out, instr.rs() as u32);
        let imm = if upper {
            (instr.uimm() as u32) << 16
        } else {
            instr.uimm() as u32
        };
        emit_i32_const(out, imm as i32);
        out.push(op);
        emit_local_set(out, instr.ra() as u32);
    }

    fn emit_andi_dot_like(&self, out: &mut Vec<u8>, instr: Instruction, upper: bool) {
        emit_local_get(out, instr.rs() as u32);
        let mask = if upper {
            (instr.uimm() as u32) << 16
        } else {
            instr.uimm() as u32
        };
        emit_i32_const(out, mask as i32);
        out.push(0x71); // i32.and
        emit_local_set(out, instr.ra() as u32);

        self.emit_update_cr0_from_reg(out, instr.ra() as u32);
    }

    fn emit_lwz(&self, out: &mut Vec<u8>, instr: Instruction) {
        self.emit_effective_address(out, instr.ra(), instr.disp());
        emit_local_set(out, CMP_LHS_LOCAL_INDEX);

        self.emit_fastmem_mem1_phys(out, CMP_LHS_LOCAL_INDEX, CMP_RHS_LOCAL_INDEX);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        emit_i32_const(out, RAM_END.wrapping_sub(3) as i32);
        out.push(0x4c); // i32.le_u
        out.push(0x04); // if
        out.push(0x40);

        emit_i32_const(out, self.runtime_mem1_base() as i32);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        out.push(0x6a); // i32.add
        out.push(0x28); // i32.load
        write_uleb(out, 2);
        write_uleb(out, 0);
        emit_i32_bswap(out);
        emit_local_set(out, instr.rd() as u32);
        out.push(0x05); // else
        out.push(0x40);

        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_call(out, READ_U32_IMPORT_INDEX);
        emit_local_set(out, instr.rd() as u32);
        out.push(0x0b); // end if
    }

    fn emit_lwzu(&self, out: &mut Vec<u8>, instr: Instruction) {
        self.emit_lwz(out, instr);
        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_local_set(out, instr.ra() as u32);
    }

    fn emit_lbz(&self, out: &mut Vec<u8>, instr: Instruction) {
        self.emit_effective_address(out, instr.ra(), instr.disp());
        emit_local_set(out, CMP_LHS_LOCAL_INDEX);

        self.emit_fastmem_mem1_phys(out, CMP_LHS_LOCAL_INDEX, CMP_RHS_LOCAL_INDEX);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        emit_i32_const(out, RAM_END as i32);
        out.push(0x4c); // i32.le_u
        out.push(0x04); // if
        out.push(0x40);

        emit_i32_const(out, self.runtime_mem1_base() as i32);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        out.push(0x6a); // i32.add
        emit_i32_load8_u_from_stack(out);
        emit_local_set(out, instr.rd() as u32);
        out.push(0x05); // else
        out.push(0x40);

        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_call(out, READ_U8_IMPORT_INDEX);
        emit_local_set(out, instr.rd() as u32);
        out.push(0x0b); // end if
    }

    fn emit_lbzu(&self, out: &mut Vec<u8>, instr: Instruction) {
        self.emit_lbz(out, instr);
        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_local_set(out, instr.ra() as u32);
    }

    fn emit_lhz(&self, out: &mut Vec<u8>, instr: Instruction) {
        self.emit_effective_address(out, instr.ra(), instr.disp());
        emit_local_set(out, CMP_LHS_LOCAL_INDEX);

        self.emit_fastmem_mem1_phys(out, CMP_LHS_LOCAL_INDEX, CMP_RHS_LOCAL_INDEX);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        emit_i32_const(out, RAM_END.wrapping_sub(1) as i32);
        out.push(0x4c); // i32.le_u
        out.push(0x04); // if
        out.push(0x40);

        emit_i32_const(out, self.runtime_mem1_base() as i32);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        out.push(0x6a); // i32.add
        emit_i32_load8_u_from_stack(out);
        emit_i32_const(out, 8);
        out.push(0x74); // i32.shl

        emit_i32_const(out, self.runtime_mem1_base() as i32);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        emit_i32_const(out, 1);
        out.push(0x6a); // i32.add
        out.push(0x6a); // i32.add
        emit_i32_load8_u_from_stack(out);
        out.push(0x72); // i32.or
        emit_local_set(out, instr.rd() as u32);
        out.push(0x05); // else
        out.push(0x40);

        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_call(out, READ_U16_IMPORT_INDEX);
        emit_local_set(out, instr.rd() as u32);
        out.push(0x0b); // end if
    }

    fn emit_lhzu(&self, out: &mut Vec<u8>, instr: Instruction) {
        self.emit_lhz(out, instr);
        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_local_set(out, instr.ra() as u32);
    }

    fn emit_lha(&self, out: &mut Vec<u8>, instr: Instruction) {
        self.emit_lhz(out, instr);
        emit_local_get(out, instr.rd() as u32);
        emit_i32_const(out, 16);
        out.push(0x74); // i32.shl
        emit_i32_const(out, 16);
        out.push(0x75); // i32.shr_s
        emit_local_set(out, instr.rd() as u32);
    }

    fn emit_lhau(&self, out: &mut Vec<u8>, instr: Instruction) {
        self.emit_lha(out, instr);
        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_local_set(out, instr.ra() as u32);
    }

    fn emit_stw(&self, out: &mut Vec<u8>, instr: Instruction) {
        self.emit_effective_address(out, instr.ra(), instr.disp());
        emit_local_set(out, CMP_LHS_LOCAL_INDEX);

        self.emit_fastmem_mem1_phys(out, CMP_LHS_LOCAL_INDEX, CMP_RHS_LOCAL_INDEX);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        emit_i32_const(out, RAM_END.wrapping_sub(3) as i32);
        out.push(0x4c); // i32.le_u
        out.push(0x04); // if
        out.push(0x40);

        emit_i32_const(out, self.runtime_code_refcount_base() as i32);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        emit_i32_const(out, CODE_LINE_SHIFT as i32);
        out.push(0x76); // i32.shr_u
        out.push(0x6a); // i32.add
        out.push(0x2d); // i32.load8_u
        write_uleb(out, 0);
        write_uleb(out, 0);
        emit_i32_const(out, 0);
        out.push(0x47); // i32.ne
        out.push(0x04); // if
        out.push(0x40);

        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_local_get(out, instr.rs() as u32);
        emit_call(out, WRITE_U32_IMPORT_INDEX);
        out.push(0x05); // else
        out.push(0x40);

        emit_i32_const(out, self.runtime_mem1_base() as i32);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        out.push(0x6a); // i32.add
        emit_local_get(out, instr.rs() as u32);
        emit_i32_bswap(out);
        out.push(0x36); // i32.store
        write_uleb(out, 2);
        write_uleb(out, 0);
        out.push(0x0b); // end store path if
        out.push(0x05); // else
        out.push(0x40);

        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_local_get(out, instr.rs() as u32);
        emit_call(out, WRITE_U32_IMPORT_INDEX);
        out.push(0x0b); // end outer if
    }

    fn emit_stwu(&self, out: &mut Vec<u8>, instr: Instruction) {
        self.emit_stw(out, instr);
        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_local_set(out, instr.ra() as u32);
    }

    fn emit_stb(&self, out: &mut Vec<u8>, instr: Instruction) {
        self.emit_effective_address(out, instr.ra(), instr.disp());
        emit_local_set(out, CMP_LHS_LOCAL_INDEX);

        self.emit_fastmem_mem1_phys(out, CMP_LHS_LOCAL_INDEX, CMP_RHS_LOCAL_INDEX);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        emit_i32_const(out, RAM_END as i32);
        out.push(0x4c); // i32.le_u
        out.push(0x04); // if
        out.push(0x40);

        emit_i32_const(out, self.runtime_code_refcount_base() as i32);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        emit_i32_const(out, CODE_LINE_SHIFT as i32);
        out.push(0x76); // i32.shr_u
        out.push(0x6a); // i32.add
        out.push(0x2d); // i32.load8_u
        write_uleb(out, 0);
        write_uleb(out, 0);
        emit_i32_const(out, 0);
        out.push(0x47); // i32.ne
        out.push(0x04); // if
        out.push(0x40);

        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_local_get(out, instr.rs() as u32);
        emit_call(out, WRITE_U8_IMPORT_INDEX);
        out.push(0x05); // else
        out.push(0x40);

        emit_i32_const(out, self.runtime_mem1_base() as i32);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        out.push(0x6a); // i32.add
        emit_local_get(out, instr.rs() as u32);
        out.push(0x3a); // i32.store8
        write_uleb(out, 0);
        write_uleb(out, 0);
        out.push(0x0b); // end store path if
        out.push(0x05); // else
        out.push(0x40);

        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_local_get(out, instr.rs() as u32);
        emit_call(out, WRITE_U8_IMPORT_INDEX);
        out.push(0x0b); // end outer if
    }

    fn emit_stbu(&self, out: &mut Vec<u8>, instr: Instruction) {
        self.emit_stb(out, instr);
        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_local_set(out, instr.ra() as u32);
    }

    fn emit_sth(&self, out: &mut Vec<u8>, instr: Instruction) {
        self.emit_effective_address(out, instr.ra(), instr.disp());
        emit_local_set(out, CMP_LHS_LOCAL_INDEX);

        self.emit_fastmem_mem1_phys(out, CMP_LHS_LOCAL_INDEX, CMP_RHS_LOCAL_INDEX);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        emit_i32_const(out, RAM_END.wrapping_sub(1) as i32);
        out.push(0x4c); // i32.le_u
        out.push(0x04); // if
        out.push(0x40);

        emit_i32_const(out, self.runtime_code_refcount_base() as i32);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        emit_i32_const(out, CODE_LINE_SHIFT as i32);
        out.push(0x76); // i32.shr_u
        out.push(0x6a); // i32.add
        out.push(0x2d); // i32.load8_u
        write_uleb(out, 0);
        write_uleb(out, 0);
        emit_i32_const(out, 0);
        out.push(0x47); // i32.ne
        out.push(0x04); // if
        out.push(0x40);

        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_local_get(out, instr.rs() as u32);
        emit_call(out, WRITE_U16_IMPORT_INDEX);
        out.push(0x05); // else
        out.push(0x40);

        emit_i32_const(out, self.runtime_mem1_base() as i32);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        out.push(0x6a); // i32.add
        emit_local_get(out, instr.rs() as u32);
        emit_i32_const(out, 8);
        out.push(0x76); // i32.shr_u
        out.push(0x3a); // i32.store8
        write_uleb(out, 0);
        write_uleb(out, 0);

        emit_i32_const(out, self.runtime_mem1_base() as i32);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        emit_i32_const(out, 1);
        out.push(0x6a); // i32.add
        out.push(0x6a); // i32.add
        emit_local_get(out, instr.rs() as u32);
        out.push(0x3a); // i32.store8
        write_uleb(out, 0);
        write_uleb(out, 0);

        out.push(0x0b); // end store path if
        out.push(0x05); // else
        out.push(0x40);

        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_local_get(out, instr.rs() as u32);
        emit_call(out, WRITE_U16_IMPORT_INDEX);
        out.push(0x0b); // end outer if
    }

    fn emit_sthu(&self, out: &mut Vec<u8>, instr: Instruction) {
        self.emit_sth(out, instr);
        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_local_set(out, instr.ra() as u32);
    }

    fn emit_lwzx(&self, out: &mut Vec<u8>, instr: Instruction) {
        self.emit_indexed_effective_address(out, instr.ra(), instr.rb());
        emit_local_set(out, CMP_LHS_LOCAL_INDEX);

        self.emit_fastmem_mem1_phys(out, CMP_LHS_LOCAL_INDEX, CMP_RHS_LOCAL_INDEX);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        emit_i32_const(out, RAM_END.wrapping_sub(3) as i32);
        out.push(0x4c); // i32.le_u
        out.push(0x04); // if
        out.push(0x40);

        emit_i32_const(out, self.runtime_mem1_base() as i32);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        out.push(0x6a); // i32.add
        out.push(0x28); // i32.load
        write_uleb(out, 2);
        write_uleb(out, 0);
        emit_i32_bswap(out);
        emit_local_set(out, instr.rd() as u32);
        out.push(0x05); // else
        out.push(0x40);

        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_call(out, READ_U32_IMPORT_INDEX);
        emit_local_set(out, instr.rd() as u32);
        out.push(0x0b); // end if
    }

    fn emit_lwzux(&self, out: &mut Vec<u8>, instr: Instruction) {
        self.emit_lwzx(out, instr);
        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_local_set(out, instr.ra() as u32);
    }

    fn emit_lbzx(&self, out: &mut Vec<u8>, instr: Instruction) {
        self.emit_indexed_effective_address(out, instr.ra(), instr.rb());
        emit_local_set(out, CMP_LHS_LOCAL_INDEX);

        self.emit_fastmem_mem1_phys(out, CMP_LHS_LOCAL_INDEX, CMP_RHS_LOCAL_INDEX);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        emit_i32_const(out, RAM_END as i32);
        out.push(0x4c); // i32.le_u
        out.push(0x04); // if
        out.push(0x40);

        emit_i32_const(out, self.runtime_mem1_base() as i32);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        out.push(0x6a); // i32.add
        emit_i32_load8_u_from_stack(out);
        emit_local_set(out, instr.rd() as u32);
        out.push(0x05); // else
        out.push(0x40);

        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_call(out, READ_U8_IMPORT_INDEX);
        emit_local_set(out, instr.rd() as u32);
        out.push(0x0b); // end if
    }

    fn emit_lbzux(&self, out: &mut Vec<u8>, instr: Instruction) {
        self.emit_lbzx(out, instr);
        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_local_set(out, instr.ra() as u32);
    }

    fn emit_lhzx(&self, out: &mut Vec<u8>, instr: Instruction) {
        self.emit_indexed_effective_address(out, instr.ra(), instr.rb());
        emit_local_set(out, CMP_LHS_LOCAL_INDEX);

        self.emit_fastmem_mem1_phys(out, CMP_LHS_LOCAL_INDEX, CMP_RHS_LOCAL_INDEX);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        emit_i32_const(out, RAM_END.wrapping_sub(1) as i32);
        out.push(0x4c); // i32.le_u
        out.push(0x04); // if
        out.push(0x40);

        emit_i32_const(out, self.runtime_mem1_base() as i32);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        out.push(0x6a); // i32.add
        emit_i32_load8_u_from_stack(out);
        emit_i32_const(out, 8);
        out.push(0x74); // i32.shl

        emit_i32_const(out, self.runtime_mem1_base() as i32);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        emit_i32_const(out, 1);
        out.push(0x6a); // i32.add
        out.push(0x6a); // i32.add
        emit_i32_load8_u_from_stack(out);
        out.push(0x72); // i32.or
        emit_local_set(out, instr.rd() as u32);
        out.push(0x05); // else
        out.push(0x40);

        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_call(out, READ_U16_IMPORT_INDEX);
        emit_local_set(out, instr.rd() as u32);
        out.push(0x0b); // end if
    }

    fn emit_lhzux(&self, out: &mut Vec<u8>, instr: Instruction) {
        self.emit_lhzx(out, instr);
        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_local_set(out, instr.ra() as u32);
    }

    fn emit_stwx(&self, out: &mut Vec<u8>, instr: Instruction) {
        self.emit_indexed_effective_address(out, instr.ra(), instr.rb());
        emit_local_set(out, CMP_LHS_LOCAL_INDEX);

        self.emit_fastmem_mem1_phys(out, CMP_LHS_LOCAL_INDEX, CMP_RHS_LOCAL_INDEX);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        emit_i32_const(out, RAM_END.wrapping_sub(3) as i32);
        out.push(0x4c); // i32.le_u
        out.push(0x04); // if
        out.push(0x40);

        emit_i32_const(out, self.runtime_code_refcount_base() as i32);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        emit_i32_const(out, CODE_LINE_SHIFT as i32);
        out.push(0x76); // i32.shr_u
        out.push(0x6a); // i32.add
        out.push(0x2d); // i32.load8_u
        write_uleb(out, 0);
        write_uleb(out, 0);
        emit_i32_const(out, 0);
        out.push(0x47); // i32.ne
        out.push(0x04); // if
        out.push(0x40);

        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_local_get(out, instr.rs() as u32);
        emit_call(out, WRITE_U32_IMPORT_INDEX);
        out.push(0x05); // else
        out.push(0x40);

        emit_i32_const(out, self.runtime_mem1_base() as i32);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        out.push(0x6a); // i32.add
        emit_local_get(out, instr.rs() as u32);
        emit_i32_bswap(out);
        out.push(0x36); // i32.store
        write_uleb(out, 2);
        write_uleb(out, 0);
        out.push(0x0b); // end store path if
        out.push(0x05); // else
        out.push(0x40);

        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_local_get(out, instr.rs() as u32);
        emit_call(out, WRITE_U32_IMPORT_INDEX);
        out.push(0x0b); // end outer if
    }

    fn emit_stwux(&self, out: &mut Vec<u8>, instr: Instruction) {
        self.emit_stwx(out, instr);
        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_local_set(out, instr.ra() as u32);
    }

    fn emit_stbx(&self, out: &mut Vec<u8>, instr: Instruction) {
        self.emit_indexed_effective_address(out, instr.ra(), instr.rb());
        emit_local_set(out, CMP_LHS_LOCAL_INDEX);

        self.emit_fastmem_mem1_phys(out, CMP_LHS_LOCAL_INDEX, CMP_RHS_LOCAL_INDEX);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        emit_i32_const(out, RAM_END as i32);
        out.push(0x4c); // i32.le_u
        out.push(0x04); // if
        out.push(0x40);

        emit_i32_const(out, self.runtime_code_refcount_base() as i32);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        emit_i32_const(out, CODE_LINE_SHIFT as i32);
        out.push(0x76); // i32.shr_u
        out.push(0x6a); // i32.add
        out.push(0x2d); // i32.load8_u
        write_uleb(out, 0);
        write_uleb(out, 0);
        emit_i32_const(out, 0);
        out.push(0x47); // i32.ne
        out.push(0x04); // if
        out.push(0x40);

        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_local_get(out, instr.rs() as u32);
        emit_call(out, WRITE_U8_IMPORT_INDEX);
        out.push(0x05); // else
        out.push(0x40);

        emit_i32_const(out, self.runtime_mem1_base() as i32);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        out.push(0x6a); // i32.add
        emit_local_get(out, instr.rs() as u32);
        out.push(0x3a); // i32.store8
        write_uleb(out, 0);
        write_uleb(out, 0);
        out.push(0x0b); // end store path if
        out.push(0x05); // else
        out.push(0x40);

        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_local_get(out, instr.rs() as u32);
        emit_call(out, WRITE_U8_IMPORT_INDEX);
        out.push(0x0b); // end outer if
    }

    fn emit_stbux(&self, out: &mut Vec<u8>, instr: Instruction) {
        self.emit_stbx(out, instr);
        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_local_set(out, instr.ra() as u32);
    }

    fn emit_sthx(&self, out: &mut Vec<u8>, instr: Instruction) {
        self.emit_indexed_effective_address(out, instr.ra(), instr.rb());
        emit_local_set(out, CMP_LHS_LOCAL_INDEX);

        self.emit_fastmem_mem1_phys(out, CMP_LHS_LOCAL_INDEX, CMP_RHS_LOCAL_INDEX);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        emit_i32_const(out, RAM_END.wrapping_sub(1) as i32);
        out.push(0x4c); // i32.le_u
        out.push(0x04); // if
        out.push(0x40);

        emit_i32_const(out, self.runtime_code_refcount_base() as i32);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        emit_i32_const(out, CODE_LINE_SHIFT as i32);
        out.push(0x76); // i32.shr_u
        out.push(0x6a); // i32.add
        out.push(0x2d); // i32.load8_u
        write_uleb(out, 0);
        write_uleb(out, 0);
        emit_i32_const(out, 0);
        out.push(0x47); // i32.ne
        out.push(0x04); // if
        out.push(0x40);

        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_local_get(out, instr.rs() as u32);
        emit_call(out, WRITE_U16_IMPORT_INDEX);
        out.push(0x05); // else
        out.push(0x40);

        emit_i32_const(out, self.runtime_mem1_base() as i32);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        out.push(0x6a); // i32.add
        emit_local_get(out, instr.rs() as u32);
        emit_i32_const(out, 8);
        out.push(0x76); // i32.shr_u
        out.push(0x3a); // i32.store8
        write_uleb(out, 0);
        write_uleb(out, 0);

        emit_i32_const(out, self.runtime_mem1_base() as i32);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        emit_i32_const(out, 1);
        out.push(0x6a); // i32.add
        out.push(0x6a); // i32.add
        emit_local_get(out, instr.rs() as u32);
        out.push(0x3a); // i32.store8
        write_uleb(out, 0);
        write_uleb(out, 0);

        out.push(0x0b); // end store path if
        out.push(0x05); // else
        out.push(0x40);

        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_local_get(out, instr.rs() as u32);
        emit_call(out, WRITE_U16_IMPORT_INDEX);
        out.push(0x0b); // end outer if
    }

    fn emit_sthux(&self, out: &mut Vec<u8>, instr: Instruction) {
        self.emit_sthx(out, instr);
        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_local_set(out, instr.ra() as u32);
    }

    fn emit_indexed_effective_address(&self, out: &mut Vec<u8>, ra: u8, rb: u8) {
        if ra == 0 {
            emit_i32_const(out, 0);
        } else {
            emit_local_get(out, ra as u32);
        }
        emit_local_get(out, rb as u32);
        out.push(0x6a); // i32.add
    }

    fn emit_effective_address(&self, out: &mut Vec<u8>, ra: u8, disp: i32) {
        if ra == 0 {
            emit_i32_const(out, 0);
        } else {
            emit_local_get(out, ra as u32);
        }
        emit_i32_const(out, disp);
        out.push(0x6a); // i32.add
    }

    fn emit_fastmem_mem1_phys(&self, out: &mut Vec<u8>, ea_local_index: u32, phys_local_index: u32) {
        emit_local_get(out, ea_local_index);
        emit_i32_const(out, 0x3FFF_FFFFu32 as i32);
        out.push(0x71); // i32.and
        emit_local_set(out, phys_local_index);
    }

    fn emit_cmp_imm(&self, out: &mut Vec<u8>, instr: Instruction, signed: bool) {
        emit_local_get(out, instr.ra() as u32);
        emit_local_set(out, CMP_LHS_LOCAL_INDEX);

        let imm = if signed {
            instr.simm()
        } else {
            instr.uimm() as i32
        };
        emit_i32_const(out, imm);
        emit_local_set(out, CMP_RHS_LOCAL_INDEX);

        self.emit_update_cr_field_from_cmp(out, instr.crfd(), signed);
    }

    fn emit_cmp_xform(&self, out: &mut Vec<u8>, instr: Instruction, signed: bool) {
        emit_local_get(out, instr.ra() as u32);
        emit_local_set(out, CMP_LHS_LOCAL_INDEX);
        emit_local_get(out, instr.rb() as u32);
        emit_local_set(out, CMP_RHS_LOCAL_INDEX);

        self.emit_update_cr_field_from_cmp(out, instr.crfd(), signed);
    }

    fn emit_update_cr_field_from_cmp(&self, out: &mut Vec<u8>, crfd: u8, signed: bool) {
        let shift = 28 - 4 * u32::from(crfd);
        let clear_mask = !(0xFu32 << shift);

        emit_local_get(out, CR_PARAM_INDEX);
        emit_i32_const(out, clear_mask as i32);
        out.push(0x71); // i32.and

        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        out.push(if signed { 0x48 } else { 0x49 }); // i32.lt_s / i32.lt_u
        emit_i32_const(out, 3);
        out.push(0x74); // i32.shl

        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        out.push(if signed { 0x4a } else { 0x4b }); // i32.gt_s / i32.gt_u
        emit_i32_const(out, 2);
        out.push(0x74); // i32.shl
        out.push(0x72); // i32.or

        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        emit_local_get(out, CMP_RHS_LOCAL_INDEX);
        out.push(0x46); // i32.eq
        emit_i32_const(out, 1);
        out.push(0x74); // i32.shl
        out.push(0x72); // i32.or

        emit_local_get(out, SO_PARAM_INDEX);
        out.push(0x72); // i32.or

        if shift != 0 {
            emit_i32_const(out, shift as i32);
            out.push(0x74); // i32.shl
        }

        emit_local_set(out, CR_PARAM_INDEX);
    }

    fn emit_update_cr0_from_reg(&self, out: &mut Vec<u8>, reg_local_index: u32) {
        emit_local_get(out, CR_PARAM_INDEX);
        emit_i32_const(out, !(0xFu32 << 28) as i32);
        out.push(0x71); // i32.and

        emit_local_get(out, reg_local_index);
        emit_i32_const(out, 0);
        out.push(0x48); // i32.lt_s
        emit_i32_const(out, 3);
        out.push(0x74); // i32.shl

        emit_local_get(out, reg_local_index);
        emit_i32_const(out, 0);
        out.push(0x4a); // i32.gt_s
        emit_i32_const(out, 2);
        out.push(0x74); // i32.shl
        out.push(0x72); // i32.or

        emit_local_get(out, reg_local_index);
        emit_i32_const(out, 0);
        out.push(0x46); // i32.eq
        emit_i32_const(out, 1);
        out.push(0x74); // i32.shl
        out.push(0x72); // i32.or

        emit_local_get(out, SO_PARAM_INDEX);
        out.push(0x72); // i32.or
        out.push(0x72); // i32.or
        emit_local_set(out, CR_PARAM_INDEX);
    }

    fn emit_bc_terminator(&self, out: &mut Vec<u8>, instr: Instruction, branch_pc: u32) {
        let bo = instr.bo();
        let decrement_ctr = (bo & 0x04) == 0;
        if decrement_ctr {
            emit_local_get(out, CTR_PARAM_INDEX);
            emit_i32_const(out, -1);
            out.push(0x6a); // i32.add
            emit_local_set(out, CTR_PARAM_INDEX);
        }

        let target = if instr.aa() {
            instr.bd() as u32
        } else {
            branch_pc.wrapping_add_signed(instr.bd())
        };
        let fallthrough = branch_pc.wrapping_add(4);

        if decrement_ctr {
            emit_local_get(out, CTR_PARAM_INDEX);
            emit_i32_const(out, 0);
            out.push(if (bo & 0x02) != 0 { 0x46 } else { 0x47 }); // eq / ne
        } else {
            emit_i32_const(out, 1);
        }

        if (bo & 0x10) != 0 {
            emit_i32_const(out, 1);
        } else {
            emit_local_get(out, CR_PARAM_INDEX);
            emit_i32_const(out, (31 - instr.bi()) as i32);
            out.push(0x76); // i32.shr_u
            emit_i32_const(out, 1);
            out.push(0x71); // i32.and
            emit_i32_const(out, if (bo & 0x08) != 0 { 1 } else { 0 });
            out.push(0x46); // i32.eq
        }

        out.push(0x71); // i32.and
        emit_local_set(out, CMP_LHS_LOCAL_INDEX);
        emit_i32_const(out, target as i32);
        emit_i32_const(out, fallthrough as i32);
        emit_local_get(out, CMP_LHS_LOCAL_INDEX);
        out.push(0x1b); // select
        emit_local_set(out, NEXT_PC_LOCAL_INDEX);
    }

    fn emit_branch_to_reg_terminator(&self, out: &mut Vec<u8>, instr: Instruction, branch_pc: u32) {
        let bo = instr.bo();
        let branch_xo = xo10(instr);
        let fallthrough = branch_pc.wrapping_add(4);

        match branch_xo {
            16 => {
                let decrement_ctr = (bo & 0x04) == 0;
                if decrement_ctr {
                    emit_local_get(out, CTR_PARAM_INDEX);
                    emit_i32_const(out, -1);
                    out.push(0x6a); // i32.add
                    emit_local_set(out, CTR_PARAM_INDEX);
                }

                if decrement_ctr {
                    emit_local_get(out, CTR_PARAM_INDEX);
                    emit_i32_const(out, 0);
                    out.push(if (bo & 0x02) != 0 { 0x46 } else { 0x47 }); // eq / ne
                } else {
                    emit_i32_const(out, 1);
                }

                if (bo & 0x10) != 0 {
                    emit_i32_const(out, 1);
                } else {
                    emit_local_get(out, CR_PARAM_INDEX);
                    emit_i32_const(out, (31 - instr.bi()) as i32);
                    out.push(0x76); // i32.shr_u
                    emit_i32_const(out, 1);
                    out.push(0x71); // i32.and
                    emit_i32_const(out, if (bo & 0x08) != 0 { 1 } else { 0 });
                    out.push(0x46); // i32.eq
                }

                out.push(0x71); // i32.and
                emit_local_set(out, CMP_LHS_LOCAL_INDEX);

                if instr.lk() {
                    emit_i32_const(out, branch_pc.wrapping_add(4) as i32);
                    emit_local_get(out, LR_LOCAL_INDEX);
                    emit_local_get(out, CMP_LHS_LOCAL_INDEX);
                    out.push(0x1b); // select
                    emit_local_set(out, LR_LOCAL_INDEX);
                }

                emit_local_get(out, LR_LOCAL_INDEX);
                emit_i32_const(out, !3);
                out.push(0x71); // i32.and
                emit_i32_const(out, fallthrough as i32);
                emit_local_get(out, CMP_LHS_LOCAL_INDEX);
                out.push(0x1b); // select
                emit_local_set(out, NEXT_PC_LOCAL_INDEX);
            }
            528 => {
                if (bo & 0x10) != 0 {
                    emit_i32_const(out, 1);
                } else {
                    emit_local_get(out, CR_PARAM_INDEX);
                    emit_i32_const(out, (31 - instr.bi()) as i32);
                    out.push(0x76); // i32.shr_u
                    emit_i32_const(out, 1);
                    out.push(0x71); // i32.and
                    emit_i32_const(out, if (bo & 0x08) != 0 { 1 } else { 0 });
                    out.push(0x46); // i32.eq
                }

                emit_local_set(out, CMP_LHS_LOCAL_INDEX);

                if instr.lk() {
                    emit_i32_const(out, branch_pc.wrapping_add(4) as i32);
                    emit_local_get(out, LR_LOCAL_INDEX);
                    emit_local_get(out, CMP_LHS_LOCAL_INDEX);
                    out.push(0x1b); // select
                    emit_local_set(out, LR_LOCAL_INDEX);
                }

                emit_local_get(out, CTR_PARAM_INDEX);
                emit_i32_const(out, !3);
                out.push(0x71); // i32.and
                emit_i32_const(out, fallthrough as i32);
                emit_local_get(out, CMP_LHS_LOCAL_INDEX);
                out.push(0x1b); // select
                emit_local_set(out, NEXT_PC_LOCAL_INDEX);
            }
            _ => unreachable!("unsupported branch-to-register terminator reached lowering"),
        }
    }
}

fn fallback_reason_sort_key(reason: UnsupportedOpcode) -> (u8, u32) {
    match reason {
        UnsupportedOpcode::Primary(op) => (0, u32::from(op)),
        UnsupportedOpcode::Xo10(xo) => (1, xo),
        UnsupportedOpcode::Mtspr(spr) => (2, u32::from(spr)),
        UnsupportedOpcode::Terminator(term) => (3, term as u32),
        UnsupportedOpcode::Unprofitable { instr_count } => (4, u32::from(instr_count)),
    }
}

fn rlwinm_mask(mb: u32, me: u32) -> u32 {
    let begin = 0xFFFF_FFFFu32 >> mb;
    let end = if me >= 31 { 0 } else { 0xFFFF_FFFFu32 >> (me + 1) };
    if mb <= me { begin & !end } else { begin | !end }
}

fn primary_opcode_label(op: u8) -> &'static str {
    match op {
        10 => "cmpli",
        11 => "cmpi",
        14 => "addi",
        15 => "addis",
        16 => "bc",
        18 => "b",
        19 => "bclr/bcctr/misc19",
        21 => "rlwinm",
        24 => "ori",
        25 => "oris",
        26 => "xori",
        27 => "xoris",
        28 => "andi.",
        29 => "andis.",
        31 => "xform31",
        32 => "lwz",
        33 => "lwzu",
        34 => "lbz",
        35 => "lbzu",
        36 => "stw",
        37 => "stwu",
        38 => "stb",
        39 => "stbu",
        40 => "lhz",
        41 => "lhzu",
        42 => "lha",
        43 => "lhau",
        44 => "sth",
        45 => "sthu",
        46 => "lmw",
        47 => "stmw",
        _ => "op",
    }
}

fn terminator_label(term: TermKind) -> &'static str {
    match term {
        TermKind::Branch => "b-term",
        TermKind::BranchLink => "bl-term",
        TermKind::BranchCond => "bc-term",
        TermKind::BranchToReg => "breg-term",
        TermKind::SystemCall => "sc-term",
        TermKind::Rfi => "rfi-term",
        TermKind::Mtmsr => "mtmsr-term",
        TermKind::Mtspr => "mtspr-term",
        TermKind::Isync => "isync-term",
        TermKind::LengthCap => "len-cap",
    }
}

#[cfg(target_arch = "wasm32")]
fn active_runtime_wasm_system_slot<const SYSTEM: SystemId>() -> &'static AtomicUsize {
    if SYSTEM == crate::system::GC {
        &ACTIVE_RUNTIME_WASM_SYSTEM_GC
    } else {
        &ACTIVE_RUNTIME_WASM_SYSTEM_WII
    }
}

#[cfg(target_arch = "wasm32")]
fn with_active_runtime_wasm_system<const SYSTEM: SystemId, R>(
    sys: *mut crate::system::System<SYSTEM>,
    f: impl FnOnce() -> R,
) -> R {
    let slot = active_runtime_wasm_system_slot::<SYSTEM>();
    let previous = slot.swap(sys.cast::<()>() as usize, Ordering::Relaxed);
    let result = f();
    slot.store(previous, Ordering::Relaxed);
    result
}

#[cfg(target_arch = "wasm32")]
fn runtime_wasm_read_u32<const SYSTEM: SystemId>(ea: u32) -> u32 {
    let ptr = active_runtime_wasm_system_slot::<SYSTEM>().load(Ordering::Relaxed) as *mut crate::system::System<SYSTEM>;
    debug_assert!(!ptr.is_null(), "runtime-wasm read_u32 called without active system");
    unsafe { (*ptr).read_u32(ea) }
}

#[cfg(target_arch = "wasm32")]
fn runtime_wasm_read_u16<const SYSTEM: SystemId>(ea: u32) -> u32 {
    let ptr = active_runtime_wasm_system_slot::<SYSTEM>().load(Ordering::Relaxed) as *mut crate::system::System<SYSTEM>;
    debug_assert!(!ptr.is_null(), "runtime-wasm read_u16 called without active system");
    unsafe { (*ptr).read_u16(ea) as u32 }
}

#[cfg(target_arch = "wasm32")]
fn runtime_wasm_read_u8<const SYSTEM: SystemId>(ea: u32) -> u32 {
    let ptr = active_runtime_wasm_system_slot::<SYSTEM>().load(Ordering::Relaxed) as *mut crate::system::System<SYSTEM>;
    debug_assert!(!ptr.is_null(), "runtime-wasm read_u8 called without active system");
    unsafe { (*ptr).read_u8(ea) as u32 }
}

#[cfg(target_arch = "wasm32")]
fn runtime_wasm_write_u32<const SYSTEM: SystemId>(ea: u32, value: u32) {
    let ptr = active_runtime_wasm_system_slot::<SYSTEM>().load(Ordering::Relaxed) as *mut crate::system::System<SYSTEM>;
    debug_assert!(!ptr.is_null(), "runtime-wasm write_u32 called without active system");
    unsafe { (*ptr).write_u32(ea, value) };
}

#[cfg(target_arch = "wasm32")]
fn runtime_wasm_write_u16<const SYSTEM: SystemId>(ea: u32, value: u32) {
    let ptr = active_runtime_wasm_system_slot::<SYSTEM>().load(Ordering::Relaxed) as *mut crate::system::System<SYSTEM>;
    debug_assert!(!ptr.is_null(), "runtime-wasm write_u16 called without active system");
    unsafe { (*ptr).write_u16(ea, value as u16) };
}

#[cfg(target_arch = "wasm32")]
fn runtime_wasm_write_u8<const SYSTEM: SystemId>(ea: u32, value: u32) {
    let ptr = active_runtime_wasm_system_slot::<SYSTEM>().load(Ordering::Relaxed) as *mut crate::system::System<SYSTEM>;
    debug_assert!(!ptr.is_null(), "runtime-wasm write_u8 called without active system");
    unsafe { (*ptr).write_u8(ea, value as u8) };
}

#[cfg(target_arch = "wasm32")]
fn make_runtime_wasm_read_u32_import<const SYSTEM: SystemId>() -> Closure<dyn FnMut(u32) -> u32> {
    Closure::wrap(Box::new(|ea: u32| runtime_wasm_read_u32::<SYSTEM>(ea)) as Box<dyn FnMut(u32) -> u32>)
}

#[cfg(target_arch = "wasm32")]
fn make_runtime_wasm_read_u16_import<const SYSTEM: SystemId>() -> Closure<dyn FnMut(u32) -> u32> {
    Closure::wrap(Box::new(|ea: u32| runtime_wasm_read_u16::<SYSTEM>(ea)) as Box<dyn FnMut(u32) -> u32>)
}

#[cfg(target_arch = "wasm32")]
fn make_runtime_wasm_read_u8_import<const SYSTEM: SystemId>() -> Closure<dyn FnMut(u32) -> u32> {
    Closure::wrap(Box::new(|ea: u32| runtime_wasm_read_u8::<SYSTEM>(ea)) as Box<dyn FnMut(u32) -> u32>)
}

#[cfg(target_arch = "wasm32")]
fn make_runtime_wasm_write_u32_import<const SYSTEM: SystemId>() -> Closure<dyn FnMut(u32, u32)> {
    Closure::wrap(Box::new(|ea: u32, value: u32| runtime_wasm_write_u32::<SYSTEM>(ea, value)) as Box<dyn FnMut(u32, u32)>)
}

#[cfg(target_arch = "wasm32")]
fn make_runtime_wasm_write_u16_import<const SYSTEM: SystemId>() -> Closure<dyn FnMut(u32, u32)> {
    Closure::wrap(Box::new(|ea: u32, value: u32| runtime_wasm_write_u16::<SYSTEM>(ea, value)) as Box<dyn FnMut(u32, u32)>)
}

#[cfg(target_arch = "wasm32")]
fn make_runtime_wasm_write_u8_import<const SYSTEM: SystemId>() -> Closure<dyn FnMut(u32, u32)> {
    Closure::wrap(Box::new(|ea: u32, value: u32| runtime_wasm_write_u8::<SYSTEM>(ea, value)) as Box<dyn FnMut(u32, u32)>)
}

pub fn format_fallback_reason(reason: UnsupportedOpcode) -> String {
    match reason {
        UnsupportedOpcode::Primary(op) => format!("{}({op})", primary_opcode_label(op)),
        UnsupportedOpcode::Xo10(xo) => format!("xo{xo}"),
        UnsupportedOpcode::Mtspr(spr) => format!("mtspr{spr}"),
        UnsupportedOpcode::Terminator(term) => terminator_label(term).to_string(),
        UnsupportedOpcode::Unprofitable { instr_count } => format!("tiny{instr_count}"),
    }
}

fn push_section(module: &mut Vec<u8>, id: u8, payload: &[u8]) {
    module.push(id);
    write_uleb(module, payload.len() as u32);
    module.extend_from_slice(payload);
}

fn emit_local_get(out: &mut Vec<u8>, idx: u32) {
    out.push(0x20);
    write_uleb(out, idx);
}

fn emit_local_set(out: &mut Vec<u8>, idx: u32) {
    out.push(0x21);
    write_uleb(out, idx);
}

fn emit_call(out: &mut Vec<u8>, func_idx: u32) {
    out.push(0x10);
    write_uleb(out, func_idx);
}

fn emit_i32_const(out: &mut Vec<u8>, value: i32) {
    out.push(0x41);
    write_sleb(out, value);
}

fn emit_i32_load(out: &mut Vec<u8>, addr: u32) {
    emit_i32_const(out, addr as i32);
    out.push(0x28); // i32.load
    write_uleb(out, 2); // align=4
    write_uleb(out, 0); // offset
}

fn emit_i32_load8_u_from_stack(out: &mut Vec<u8>) {
    out.push(0x2d); // i32.load8_u
    write_uleb(out, 0); // align=1
    write_uleb(out, 0); // offset
}

fn emit_state_load_local(out: &mut Vec<u8>, state_ptr: u32, state_word_index: u32, local_index: u32) {
    emit_i32_load(out, state_ptr + state_word_index * 4);
    emit_local_set(out, local_index);
}

fn emit_state_store_local(out: &mut Vec<u8>, state_ptr: u32, state_word_index: u32, local_index: u32) {
    emit_i32_const(out, (state_ptr + state_word_index * 4) as i32);
    emit_local_get(out, local_index);
    out.push(0x36); // i32.store
    write_uleb(out, 2); // align=4
    write_uleb(out, 0); // offset
}

#[cfg(target_arch = "wasm32")]
impl<const SYSTEM: SystemId> RuntimeWasmState<SYSTEM> {
    fn runtime_state_ptr(&self) -> u32 {
        self.shared.state_storage.as_ptr() as usize as u32
    }

    fn runtime_mem1_base(&self) -> u32 {
        self.shared.mem1_base
    }

    fn runtime_code_refcount_base(&self) -> u32 {
        self.shared.code_refcount_base
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<const SYSTEM: SystemId> RuntimeWasmState<SYSTEM> {
    fn runtime_state_ptr(&self) -> u32 {
        0
    }

    fn runtime_mem1_base(&self) -> u32 {
        0
    }

    fn runtime_code_refcount_base(&self) -> u32 {
        0
    }
}

fn emit_i32_bswap(out: &mut Vec<u8>) {
    out.push(0x22); // local.tee
    write_uleb(out, CMP_LHS_LOCAL_INDEX);

    emit_local_get(out, CMP_LHS_LOCAL_INDEX);
    emit_i32_const(out, 24);
    out.push(0x76); // i32.shr_u

    emit_local_get(out, CMP_LHS_LOCAL_INDEX);
    emit_i32_const(out, 0x00FF_0000);
    out.push(0x71); // i32.and
    emit_i32_const(out, 8);
    out.push(0x76); // i32.shr_u
    out.push(0x72); // i32.or

    emit_local_get(out, CMP_LHS_LOCAL_INDEX);
    emit_i32_const(out, 0x0000_FF00);
    out.push(0x71); // i32.and
    emit_i32_const(out, 8);
    out.push(0x74); // i32.shl
    out.push(0x72); // i32.or

    emit_local_get(out, CMP_LHS_LOCAL_INDEX);
    emit_i32_const(out, 24);
    out.push(0x74); // i32.shl
    out.push(0x72); // i32.or
}

fn write_uleb(out: &mut Vec<u8>, mut value: u32) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            break;
        } else {
            out.push(byte | 0x80);
        }
    }
}

fn write_sleb(out: &mut Vec<u8>, mut value: i32) {
    let mut more = true;
    while more {
        let byte = (value as u8) & 0x7f;
        value >>= 7;

        let sign_bit_set = (byte & 0x40) != 0;
        more = !((value == 0 && !sign_bit_set) || (value == -1 && sign_bit_set));

        if more {
            out.push(byte | 0x80);
        } else {
            out.push(byte);
        }
    }
}

impl<const SYSTEM: SystemId> Default for RuntimeWasmState<SYSTEM> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gekko::Gekko;
    use crate::system::GC;
    use wasmparser::Parser;

    fn runtime_addic_result_and_carry(ra: u32, simm: i16) -> (u32, bool) {
        let result = ra.wrapping_add(simm as u32);
        let carry = result < ra;
        (result, carry)
    }

    #[test]
    fn hot_counts_reach_threshold() {
        let mut state = RuntimeWasmState::<GC>::new();
        for _ in 0..HOT_BLOCK_THRESHOLD {
            state.record_block_hit(0x8000_1000);
        }

        assert!(state.should_compile(0x8000_1000));
    }

    #[test]
    fn invalidation_removes_compiled_block() {
        let mut state = RuntimeWasmState::<GC>::new();
        let fingerprint = state.fingerprint(0x8000_2000, 6, &[1, 2, 3]);
        state.register_compiled_block(
            fingerprint,
            TraceSpec {
                entry: BlockSpec {
                    start_pc: fingerprint.pc,
                    instrs: vec![0x38800001, 0x48000004],
                    pcs: vec![fingerprint.pc, fingerprint.pc.wrapping_add(4)],
                    terminator: TermKind::Branch,
                },
                successors: Vec::new(),
            },
        );

        assert!(state.lookup(&fingerprint).is_some());
        state.invalidate_pc(fingerprint.pc);
        assert!(state.lookup(&fingerprint).is_none());
        assert_eq!(state.compiled_block_count(), 0);
    }

    #[test]
    fn wasm_stub_looks_like_wasm() {
        let state = RuntimeWasmState::<GC>::new();
        let fingerprint = state.fingerprint(0x8000_4000, 1, &[0x6000_0000]);
        let spec = BlockSpec {
            start_pc: fingerprint.pc,
            instrs: vec![0x6000_0000],
            pcs: vec![fingerprint.pc],
            terminator: TermKind::LengthCap,
        };

        let bytes = state.build_wasm_module(&fingerprint, &spec);
        assert_eq!(&bytes[..4], b"\0asm");
        assert_eq!(&bytes[4..8], &1u32.to_le_bytes());
        assert!(bytes.contains(&0x20));
        assert!(bytes.contains(&0x41));
        assert!(bytes.contains(&0x6a));
    }

    #[test]
    fn lowered_module_parses_as_wasm() {
        let state = RuntimeWasmState::<GC>::new();
        let fingerprint = state.fingerprint(0x8000_5000, 2, &[0x38800001, 0x48000004]);
        let spec = BlockSpec {
            start_pc: fingerprint.pc,
            instrs: vec![0x38800001, 0x48000004],
            pcs: vec![fingerprint.pc, fingerprint.pc.wrapping_add(4)],
            terminator: TermKind::Branch,
        };

        let bytes = state.build_wasm_module(&fingerprint, &spec);
        let mut payloads = 0usize;
        for item in Parser::new(0).parse_all(&bytes) {
            item.expect("generated module should parse");
            payloads += 1;
        }

        assert!(payloads > 0);
    }

    #[test]
    fn branch_cond_module_parses_as_wasm() {
        let state = RuntimeWasmState::<GC>::new();
        let fingerprint = state.fingerprint(0x8000_6000, 2, &[0x704300FF, 0x41820008]);
        let spec = BlockSpec {
            start_pc: fingerprint.pc,
            instrs: vec![0x704300FF, 0x41820008],
            pcs: vec![fingerprint.pc, fingerprint.pc.wrapping_add(4)],
            terminator: TermKind::BranchCond,
        };

        let bytes = state.build_wasm_module(&fingerprint, &spec);
        for item in wasmparser::Parser::new(0).parse_all(&bytes) {
            item.expect("generated conditional-branch module should parse");
        }
    }

    #[test]
    fn compare_immediate_branch_module_parses_as_wasm() {
        let state = RuntimeWasmState::<GC>::new();
        let fingerprint = state.fingerprint(0x8000_6100, 2, &[0x2C030000, 0x41820008]);
        let spec = BlockSpec {
            start_pc: fingerprint.pc,
            instrs: vec![0x2C030000, 0x41820008],
            pcs: vec![fingerprint.pc, fingerprint.pc.wrapping_add(4)],
            terminator: TermKind::BranchCond,
        };

        let bytes = state.build_wasm_module(&fingerprint, &spec);
        for item in wasmparser::Parser::new(0).parse_all(&bytes) {
            item.expect("generated compare-immediate branch module should parse");
        }
    }

    #[test]
    fn compare_xform_is_compileable() {
        let state = RuntimeWasmState::<GC>::new();
        let spec = BlockSpec {
            start_pc: 0x8000_6200,
            instrs: vec![0x7C032000, 0x41820008],
            pcs: vec![0x8000_6200, 0x8000_6204],
            terminator: TermKind::BranchCond,
        };

        assert!(matches!(state.classify_block(&spec), BlockCompileDecision::Compileable(_)));
    }

    #[test]
    fn rlwinm_module_parses_as_wasm() {
        let state = RuntimeWasmState::<GC>::new();
        let fingerprint = state.fingerprint(0x8000_6230, 4, &[0x5463063E, 0x38840001, 0x60840002, 0x48000004]);
        let spec = BlockSpec {
            start_pc: fingerprint.pc,
            instrs: vec![0x5463063E, 0x38840001, 0x60840002, 0x48000004],
            pcs: vec![
                fingerprint.pc,
                fingerprint.pc.wrapping_add(4),
                fingerprint.pc.wrapping_add(8),
                fingerprint.pc.wrapping_add(12),
            ],
            terminator: TermKind::Branch,
        };

        let bytes = state.build_wasm_module(&fingerprint, &spec);
        for item in Parser::new(0).parse_all(&bytes) {
            item.expect("generated rlwinm module should parse");
        }
    }

    #[test]
    fn orx_module_parses_as_wasm() {
        let state = RuntimeWasmState::<GC>::new();
        let fingerprint = state.fingerprint(0x8000_6240, 4, &[0x7C632378, 0x38840001, 0x60840002, 0x48000004]);
        let spec = BlockSpec {
            start_pc: fingerprint.pc,
            instrs: vec![0x7C632378, 0x38840001, 0x60840002, 0x48000004],
            pcs: vec![
                fingerprint.pc,
                fingerprint.pc.wrapping_add(4),
                fingerprint.pc.wrapping_add(8),
                fingerprint.pc.wrapping_add(12),
            ],
            terminator: TermKind::Branch,
        };

        let bytes = state.build_wasm_module(&fingerprint, &spec);
        for item in Parser::new(0).parse_all(&bytes) {
            item.expect("generated orx module should parse");
        }
    }

    #[test]
    fn lwz_stw_module_parses_as_wasm() {
        let state = RuntimeWasmState::<GC>::new();
        let fingerprint = state.fingerprint(0x8000_6250, 4, &[0x80630000, 0x90640000, 0x38840004, 0x48000004]);
        let spec = BlockSpec {
            start_pc: fingerprint.pc,
            instrs: vec![0x80630000, 0x90640000, 0x38840004, 0x48000004],
            pcs: vec![
                fingerprint.pc,
                fingerprint.pc.wrapping_add(4),
                fingerprint.pc.wrapping_add(8),
                fingerprint.pc.wrapping_add(12),
            ],
            terminator: TermKind::Branch,
        };

        let bytes = state.build_wasm_module(&fingerprint, &spec);
        for item in Parser::new(0).parse_all(&bytes) {
            item.expect("generated lwz/stw module should parse");
        }
    }

    #[test]
    fn byte_halfword_module_parses_as_wasm() {
        let state = RuntimeWasmState::<GC>::new();
        let fingerprint = state.fingerprint(0x8000_6270, 6, &[0x88630000, 0xA8440002, 0x90640004, 0x98640008, 0xB064000A, 0x48000004]);
        let spec = BlockSpec {
            start_pc: fingerprint.pc,
            instrs: vec![0x88630000, 0xA8440002, 0x90640004, 0x98640008, 0xB064000A, 0x48000004],
            pcs: vec![
                fingerprint.pc,
                fingerprint.pc.wrapping_add(4),
                fingerprint.pc.wrapping_add(8),
                fingerprint.pc.wrapping_add(12),
                fingerprint.pc.wrapping_add(16),
                fingerprint.pc.wrapping_add(20),
            ],
            terminator: TermKind::Branch,
        };

        let bytes = state.build_wasm_module(&fingerprint, &spec);
        for item in Parser::new(0).parse_all(&bytes) {
            item.expect("generated byte/halfword module should parse");
        }
    }

    #[test]
    fn branch_link_falls_back_separately_from_plain_branch() {
        let state = RuntimeWasmState::<GC>::new();
        let spec = BlockSpec {
            start_pc: 0x8000_6260,
            instrs: vec![0x38800001, 0x38840002, 0x60840003, 0x48000005],
            pcs: vec![0x8000_6260, 0x8000_6264, 0x8000_6268, 0x8000_626C],
            terminator: TermKind::BranchLink,
        };

        assert!(matches!(state.classify_block(&spec), BlockCompileDecision::Compileable(_)));
    }

    #[test]
    fn successor_trace_module_parses_as_wasm() {
        let state = RuntimeWasmState::<GC>::new();
        let trace = TraceSpec {
            entry: BlockSpec {
                start_pc: 0x8000_6400,
                instrs: vec![0x2C030000, 0x41820008],
                pcs: vec![0x8000_6400, 0x8000_6404],
                terminator: TermKind::BranchCond,
            },
            successors: vec![
                BlockSpec {
                    start_pc: 0x8000_6408,
                    instrs: vec![0x38800001, 0x48000004],
                    pcs: vec![0x8000_6408, 0x8000_640C],
                    terminator: TermKind::Branch,
                },
                BlockSpec {
                    start_pc: 0x8000_6410,
                    instrs: vec![0x38800002, 0x48000004],
                    pcs: vec![0x8000_6410, 0x8000_6414],
                    terminator: TermKind::Branch,
                },
            ],
        };
        let fingerprint = state.fingerprint(
            trace.entry.start_pc,
            trace.total_instrs() as u16,
            &[
                0x2C030000, 0x41820008, 0x38800001, 0x48000004, 0x38800002, 0x48000004,
            ],
        );

        let bytes = state.build_wasm_trace_module(&fingerprint, &trace);
        for item in wasmparser::Parser::new(0).parse_all(&bytes) {
            item.expect("generated successor trace module should parse");
        }
    }

    #[test]
    fn shared_container_module_parses_as_wasm() {
        let mut state = RuntimeWasmState::<GC>::new();
        let trace_a = TraceSpec {
            entry: BlockSpec {
                start_pc: 0x8000_6500,
                instrs: vec![0x38800001, 0x48000004],
                pcs: vec![0x8000_6500, 0x8000_6504],
                terminator: TermKind::Branch,
            },
            successors: vec![BlockSpec {
                start_pc: 0x8000_6508,
                instrs: vec![0x2C030000, 0x41820008],
                pcs: vec![0x8000_6508, 0x8000_650C],
                terminator: TermKind::BranchCond,
            }],
        };
        let trace_b = TraceSpec {
            entry: BlockSpec {
                start_pc: 0x8000_6600,
                instrs: vec![0x38800002, 0x48000004],
                pcs: vec![0x8000_6600, 0x8000_6604],
                terminator: TermKind::Branch,
            },
            successors: Vec::new(),
        };
        let fp_a = state.fingerprint(0x8000_6500, trace_a.total_instrs() as u16, &[0x38800001, 0x48000004, 0x2C030000, 0x41820008]);
        let fp_b = state.fingerprint(0x8000_6600, trace_b.total_instrs() as u16, &[0x38800002, 0x48000004]);
        state.register_compiled_block(fp_a, trace_a);
        state.register_compiled_block(fp_b, trace_b);

        let bytes = state.build_shared_wasm_module();
        for item in Parser::new(0).parse_all(&bytes) {
            item.expect("generated shared runtime-wasm container should parse");
        }
    }

    #[test]
    fn branch_cond_module_keeps_select_opcode() {
        let state = RuntimeWasmState::<GC>::new();
        let fingerprint = state.fingerprint(0x8000_6300, 2, &[0x704300FF, 0x41820008]);
        let spec = BlockSpec {
            start_pc: fingerprint.pc,
            instrs: vec![0x704300FF, 0x41820008],
            pcs: vec![fingerprint.pc, fingerprint.pc.wrapping_add(4)],
            terminator: TermKind::BranchCond,
        };

        let bytes = state.build_wasm_module(&fingerprint, &spec);
        assert!(bytes.contains(&0x1b));
    }

    #[test]
    fn subfic_module_parses_as_wasm() {
        let state = RuntimeWasmState::<GC>::new();
        let fingerprint = state.fingerprint(0x8000_6310, 3, &[0x2084_0001, 0x3880_0002, 0x4800_0004]);
        let spec = BlockSpec {
            start_pc: fingerprint.pc,
            instrs: vec![0x2084_0001, 0x3880_0002, 0x4800_0004],
            pcs: vec![
                fingerprint.pc,
                fingerprint.pc.wrapping_add(4),
                fingerprint.pc.wrapping_add(8),
            ],
            terminator: TermKind::Branch,
        };

        let bytes = state.build_wasm_module(&fingerprint, &spec);
        for item in Parser::new(0).parse_all(&bytes) {
            item.expect("generated subfic module should parse");
        }
    }

    #[test]
    fn addic_dot_module_parses_as_wasm() {
        let state = RuntimeWasmState::<GC>::new();
        let fingerprint = state.fingerprint(0x8000_6320, 3, &[0x3484_0001, 0x3880_0002, 0x4800_0004]);
        let spec = BlockSpec {
            start_pc: fingerprint.pc,
            instrs: vec![0x3484_0001, 0x3880_0002, 0x4800_0004],
            pcs: vec![
                fingerprint.pc,
                fingerprint.pc.wrapping_add(4),
                fingerprint.pc.wrapping_add(8),
            ],
            terminator: TermKind::Branch,
        };

        let bytes = state.build_wasm_module(&fingerprint, &spec);
        for item in Parser::new(0).parse_all(&bytes) {
            item.expect("generated addic. module should parse");
        }
    }

    #[test]
    fn unsupported_primary_opcode_falls_back() {
        let mut state = RuntimeWasmState::<GC>::new();
        let spec = BlockSpec {
            start_pc: 0x8000_3000,
            instrs: vec![0xFC00_0000],
            pcs: vec![0x8000_3000],
            terminator: TermKind::LengthCap,
        };

        match state.classify_block(&spec) {
            BlockCompileDecision::Fallback { reason, .. } => {
                assert!(matches!(reason, UnsupportedOpcode::Primary(_)));
            }
            BlockCompileDecision::Compileable(_) => panic!("block should not be compileable"),
        }
    }

    #[test]
    fn addic_arithmetic_matches_interpreter_formula() {
        for (ra, simm) in [
            (0x0000_0000, 1i16),
            (0xFFFF_FFFF, 1i16),
            (0x7FFF_FFFF, -1i16),
            (0x8000_0000, -1i16),
            (0x0000_0001, -2i16),
            (0xFFFF_0000, 0x7FFF_i16),
        ] {
            let (runtime_res, runtime_carry) = runtime_addic_result_and_carry(ra, simm);
            let (interp_res, interp_carry) = ra.overflowing_add(simm as u32);
            assert_eq!(runtime_res, interp_res, "result mismatch for ra={ra:08X} simm={simm}");
            assert_eq!(runtime_carry, interp_carry, "carry mismatch for ra={ra:08X} simm={simm}");
        }
    }

    #[test]
    fn addic_dot_cr0_matches_core_update() {
        for (result, so) in [
            (0x0000_0000, false),
            (0x0000_0001, false),
            (0xFFFF_FFFF, false),
            (0x8000_0000, true),
        ] {
            let mut gekko = Gekko::new(0);
            gekko.spr.xer = gekko.spr.xer.with_summary_overflow(so);
            gekko.update_cr0(result);

            let mut runtime_cr = 0u32;
            runtime_cr |= u32::from((result as i32) < 0) << 31;
            runtime_cr |= u32::from((result as i32) > 0) << 30;
            runtime_cr |= u32::from(result == 0) << 29;
            runtime_cr |= u32::from(so) << 28;

            assert_eq!(gekko.cr.raw() & 0xF000_0000, runtime_cr, "cr0 mismatch for result={result:08X} so={so}");
        }
    }

    #[test]
    fn tiny_block_falls_back_as_unprofitable() {
        let state = RuntimeWasmState::<GC>::new();
        let spec = BlockSpec {
            start_pc: 0x8000_7000,
            instrs: vec![0x38800001, 0x48000004],
            pcs: vec![0x8000_7000, 0x8000_7004],
            terminator: TermKind::Branch,
        };

        match state.classify_block(&spec) {
            BlockCompileDecision::Fallback {
                reason: UnsupportedOpcode::Unprofitable { instr_count },
                ..
            } => assert_eq!(instr_count, 2),
            other => panic!("expected unprofitable fallback, got {other:?}"),
        }
    }
}