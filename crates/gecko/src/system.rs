use crate::HostInput;
use crate::audio::{AudioSink, EmptyAudioSink};
use crate::dvd::DvdInterface;
use crate::flipper::ai::AudioInterface;
use crate::flipper::cp::CommandProcessor;
use crate::flipper::dsp::Dsp;
use crate::flipper::exi::ExternalInterface;
use crate::flipper::gx::GraphicsProcessor;
use crate::flipper::mi::MemoryInterface;
use crate::flipper::pe::PixelEngine;
use crate::flipper::pi::ProcessorInterface;
use crate::flipper::si::SerialInterface;
use crate::flipper::vi::VideoInterface;
#[cfg(feature = "fps-counter")]
use crate::fps::FpsCounter;
use crate::gekko::Gekko;
use crate::hollywood::Hollywood;
#[cfg(feature = "hooks")]
use crate::hooks::{HookFilters, HookFlags, HookState, Host};
use crate::host::{EmptyRenderSink, RenderSink};
use crate::mmio::Mmio;
use crate::scheduler::Scheduler;
use crate::starlet::Starlet;
use image::Executable;
#[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
use std::sync::OnceLock;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;
#[cfg(target_arch = "wasm32")]
const CORE_PERF_DRAIN_EVENTS_SAMPLE_EVERY: u64 = 64;
#[cfg(target_arch = "wasm32")]
const CORE_PERF_DSP_BATCH_SAMPLE_EVERY: u64 = 64;
#[cfg(target_arch = "wasm32")]
const EVENT_DRAIN_BUDGET_PER_SLICE: usize = 1024;
#[cfg(not(target_arch = "wasm32"))]
const EVENT_DRAIN_BUDGET_PER_SLICE: usize = usize::MAX;

pub type SystemId = u8;

pub const GC: SystemId = 0;
pub const WII: SystemId = 1;

/// This only matters if `jit` feature is enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    Jit,
    Jiterpreter,
    Interpreter,
}

impl Default for ExecutionMode {
    fn default() -> Self {
        #[cfg(feature = "jit")]
        {
            Self::Jit
        }

        #[cfg(not(feature = "jit"))]
        {
            Self::Jiterpreter
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone, Copy)]
pub struct CorePerfSnapshot {
    pub run_interp_ms: f64,
    pub drain_events_ms: f64,
    pub dsp_batch_ms: f64,
    pub step_cpu_calls: u64,
    pub drain_events_calls: u64,
    pub dsp_batch_calls: u64,
    pub drain_budget_hits: u64,
    pub jiterp_blocks: u64,
    pub jiterp_instrs: u64,
    pub jiterp_fast_instrs: u64,
    pub jiterp_fallback_dispatches: u64,
    pub jiterp_fallback_by_op: [u64; 64],
    pub jiterp_fallback_op19_by_xo: [u64; 1024],
    pub jiterp_fallback_op31_by_xo: [u64; 1024],
    pub jiterp_fallback_op63_by_xo: [u64; 1024],
    pub jiterp_fallback_op63_xo89_by_subop: [u64; 16],
    pub jiterp_cache_hits: u64,
    pub jiterp_cache_misses: u64,
    pub jiterp_flushes: u64,
    pub jiterp_verify_samples: u64,
    pub jiterp_verify_mismatches: u64,
}

#[cfg(target_arch = "wasm32")]
impl Default for CorePerfSnapshot {
    fn default() -> Self {
        Self {
            run_interp_ms: 0.0,
            drain_events_ms: 0.0,
            dsp_batch_ms: 0.0,
            step_cpu_calls: 0,
            drain_events_calls: 0,
            dsp_batch_calls: 0,
            drain_budget_hits: 0,
            jiterp_blocks: 0,
            jiterp_instrs: 0,
            jiterp_fast_instrs: 0,
            jiterp_fallback_dispatches: 0,
            jiterp_fallback_by_op: [0; 64],
            jiterp_fallback_op19_by_xo: [0; 1024],
            jiterp_fallback_op31_by_xo: [0; 1024],
            jiterp_fallback_op63_by_xo: [0; 1024],
            jiterp_fallback_op63_xo89_by_subop: [0; 16],
            jiterp_cache_hits: 0,
            jiterp_cache_misses: 0,
            jiterp_flushes: 0,
            jiterp_verify_samples: 0,
            jiterp_verify_mismatches: 0,
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug)]
struct CorePerfAcc {
    run_interp_ms: f64,
    drain_events_ms: f64,
    dsp_batch_ms: f64,
    step_cpu_calls: u64,
    drain_events_calls: u64,
    dsp_batch_calls: u64,
    drain_budget_hits: u64,
    jiterp_blocks: u64,
    jiterp_instrs: u64,
    jiterp_fast_instrs: u64,
    jiterp_fallback_dispatches: u64,
    jiterp_fallback_by_op: [u64; 64],
    jiterp_fallback_op19_by_xo: [u64; 1024],
    jiterp_fallback_op31_by_xo: [u64; 1024],
    jiterp_fallback_op63_by_xo: [u64; 1024],
    jiterp_fallback_op63_xo89_by_subop: [u64; 16],
    jiterp_cache_hits: u64,
    jiterp_cache_misses: u64,
    jiterp_flushes: u64,
    jiterp_verify_samples: u64,
    jiterp_verify_mismatches: u64,
}

#[cfg(target_arch = "wasm32")]
impl Default for CorePerfAcc {
    fn default() -> Self {
        Self {
            run_interp_ms: 0.0,
            drain_events_ms: 0.0,
            dsp_batch_ms: 0.0,
            step_cpu_calls: 0,
            drain_events_calls: 0,
            dsp_batch_calls: 0,
            drain_budget_hits: 0,
            jiterp_blocks: 0,
            jiterp_instrs: 0,
            jiterp_fast_instrs: 0,
            jiterp_fallback_dispatches: 0,
            jiterp_fallback_by_op: [0; 64],
            jiterp_fallback_op19_by_xo: [0; 1024],
            jiterp_fallback_op31_by_xo: [0; 1024],
            jiterp_fallback_op63_by_xo: [0; 1024],
            jiterp_fallback_op63_xo89_by_subop: [0; 16],
            jiterp_cache_hits: 0,
            jiterp_cache_misses: 0,
            jiterp_flushes: 0,
            jiterp_verify_samples: 0,
            jiterp_verify_mismatches: 0,
        }
    }
}

pub struct System<const SYSTEM: SystemId> {
    pub vsync_pending: bool,
    pub vi_present_seen_this_frame: bool,
    pub execution_mode: ExecutionMode,
    pub gekko: Gekko,
    pub scheduler: Scheduler<SYSTEM>,
    pub mmio: Mmio<SYSTEM>,
    pub vi: VideoInterface,
    pub pe: PixelEngine,
    pub pi: ProcessorInterface,
    pub dsp: Dsp,
    pub exi: ExternalInterface,
    pub gx: GraphicsProcessor,
    pub cp: CommandProcessor,
    pub di: DvdInterface,
    pub si: SerialInterface,
    pub ai: AudioInterface,
    pub mi: MemoryInterface,

    // Wii stuff.
    pub starlet: Starlet,
    pub hollywood: Hollywood,

    /// GX dispatches actions here.
    pub render_sink: Box<dyn RenderSink>,

    /// AID DMA pushes 8-frame stereo s16 blocks here.
    pub audio_sink: Box<dyn AudioSink>,

    #[cfg(feature = "hooks")]
    pub hook_host: Option<Box<dyn Host<SYSTEM> + Send>>,
    #[cfg(feature = "hooks")]
    pub hook_flags: HookFlags,
    #[cfg(feature = "hooks")]
    pub hook_filters: HookFilters,

    #[cfg(feature = "jit")]
    pub jit: Option<Box<crate::gekko::jit::JitEngine<SYSTEM>>>,

    pub jiterpreter: Option<Box<crate::gekko::jiterp::JiterpEngine<SYSTEM>>>,
    pub jiterpreter_cache_dirty: bool,

    #[cfg(feature = "fps-counter")]
    pub fps_counter: FpsCounter,

    #[cfg(any(feature = "jit-stats", feature = "profile", feature = "gx-stats"))]
    pub heatmap: crate::profile::HeatmapConfig,

    #[cfg(any(feature = "jit-stats", feature = "profile", feature = "gx-stats"))]
    pub vsync_count: u64,

    #[cfg(feature = "profile")]
    pub pprof_config: Option<crate::profile::PprofConfig>,

    #[cfg(feature = "profile")]
    pub pprof_session: Option<crate::profile::IpSampler>,

    #[cfg(target_arch = "wasm32")]
    core_perf: CorePerfAcc,
}

#[cfg(any(all(debug_assertions, not(target_arch = "wasm32")), target_arch = "wasm32"))]
#[derive(Clone, Copy)]
struct JiterpVerifyState {
    gprs: [u32; 32],
    fprs_bits: [u64; 32],
    ps1_bits: [u64; 32],
    pc: u32,
    cia: u32,
    nia: u32,
    reserve_addr: u32,
    cr_raw: u32,
    fpscr_raw: u32,
    xer_raw: u32,
    lr: u32,
    ctr: u32,
}

impl<const SYSTEM: SystemId> System<SYSTEM> {
    #[cfg(any(all(debug_assertions, not(target_arch = "wasm32")), target_arch = "wasm32"))]
    #[inline(always)]
    fn jiterp_verify_enabled() -> bool {
        #[cfg(target_arch = "wasm32")]
        {
            false
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        *ENABLED.get_or_init(|| {
            std::env::var("GECKO_JITERP_VERIFY")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false)
        })
        }
    }

    #[cfg(any(all(debug_assertions, not(target_arch = "wasm32")), target_arch = "wasm32"))]
    #[inline(always)]
    fn jiterp_verify_sample_hit(&self, instr_raw: u32) -> bool {
        let mix = (self.scheduler.cycles as u32)
            .wrapping_mul(0x9E37_79B9)
            .rotate_left(5)
            ^ instr_raw;
        (mix & 0x7FF) == 0
    }

    #[cfg(any(all(debug_assertions, not(target_arch = "wasm32")), target_arch = "wasm32"))]
    #[inline(always)]
    fn jiterp_verify_candidate(instr_raw: u32) -> bool {
        let op = instr_raw >> 26;
        match op {
            // Register-only integer/branch/FP hot classes.
            4 | 7 | 8 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 18 | 19 | 20 | 21 | 23 | 24 | 25 | 26 | 27
            | 28 | 29 | 31 | 59 | 63 => {
                if op == 4 {
                    let subop = (instr_raw >> 1) & 0x1F;
                    // Exclude PSQ memory forms.
                    return subop != 6 && subop != 7;
                }
                if op == 19 {
                    let xo10 = (instr_raw >> 1) & 0x3FF;
                    // bclr / bcctr only.
                    return xo10 == 16 || xo10 == 528;
                }
                if op == 31 {
                    let xo10 = (instr_raw >> 1) & 0x3FF;
                    // Exclude op31 memory and privileged/system forms.
                    return matches!(
                        xo10,
                        0
                            | 8
                            | 10
                            | 11
                            | 24
                            | 26
                            | 28
                            | 32
                            | 40
                            | 60
                            | 75
                            | 104
                            | 136
                            | 138
                            | 200
                            | 202
                            | 232
                            | 234
                            | 235
                            | 266
                            | 284
                            | 316
                            | 412
                            | 444
                            | 459
                            | 476
                            | 491
                            | 536
                            | 792
                            | 824
                            | 922
                            | 954
                    );
                }
                true
            }
            _ => false,
        }
    }

    #[cfg(any(all(debug_assertions, not(target_arch = "wasm32")), target_arch = "wasm32"))]
    #[inline(always)]
    fn jiterp_capture_state(&self) -> JiterpVerifyState {
        JiterpVerifyState {
            gprs: self.gekko.gprs,
            fprs_bits: std::array::from_fn(|i| self.gekko.fprs[i].to_bits()),
            ps1_bits: std::array::from_fn(|i| self.gekko.ps1s[i].to_bits()),
            pc: self.gekko.pc,
            cia: self.gekko.cia,
            nia: self.gekko.nia,
            reserve_addr: self.gekko.reserve_addr,
            cr_raw: self.gekko.cr.raw(),
            fpscr_raw: self.gekko.fpscr.raw(),
            xer_raw: self.gekko.spr.xer.raw(),
            lr: self.gekko.spr.lr,
            ctr: self.gekko.spr.ctr,
        }
    }

    #[cfg(any(all(debug_assertions, not(target_arch = "wasm32")), target_arch = "wasm32"))]
    #[inline(always)]
    fn jiterp_restore_state(&mut self, s: &JiterpVerifyState) {
        self.gekko.gprs = s.gprs;
        self.gekko.fprs = std::array::from_fn(|i| f64::from_bits(s.fprs_bits[i]));
        self.gekko.ps1s = std::array::from_fn(|i| f64::from_bits(s.ps1_bits[i]));
        self.gekko.pc = s.pc;
        self.gekko.cia = s.cia;
        self.gekko.nia = s.nia;
        self.gekko.reserve_addr = s.reserve_addr;
        self.gekko.cr = crate::gekko::condition::ConditionRegister::from(s.cr_raw);
        self.gekko.fpscr = crate::gekko::fpscr::Fpscr::from(s.fpscr_raw);
        self.gekko.spr.xer = crate::gekko::spr::Xer::from(s.xer_raw);
        self.gekko.spr.lr = s.lr;
        self.gekko.spr.ctr = s.ctr;
    }

    #[cfg(any(all(debug_assertions, not(target_arch = "wasm32")), target_arch = "wasm32"))]
    #[inline(always)]
    fn jiterp_state_matches(a: &JiterpVerifyState, b: &JiterpVerifyState) -> bool {
        a.gprs == b.gprs
            && a.fprs_bits == b.fprs_bits
            && a.ps1_bits == b.ps1_bits
            && a.pc == b.pc
            && a.cia == b.cia
            && a.nia == b.nia
            && a.reserve_addr == b.reserve_addr
            && a.cr_raw == b.cr_raw
            && a.fpscr_raw == b.fpscr_raw
            && a.xer_raw == b.xer_raw
            && a.lr == b.lr
            && a.ctr == b.ctr
    }

    #[cfg(any(all(debug_assertions, not(target_arch = "wasm32")), target_arch = "wasm32"))]
    #[inline(always)]
    fn jiterp_verify_fast_vs_interp(
        &mut self,
        instr_raw: u32,
        pre_state: &JiterpVerifyState,
        fast_post_state: &JiterpVerifyState,
    ) {
        self.jiterp_restore_state(pre_state);
        let instr = crate::gekko::instruction::Instruction(instr_raw);
        crate::gekko::dispatch(self, instr);
        let interp_post_state = self.jiterp_capture_state();

        #[cfg(target_arch = "wasm32")]
        {
            self.core_perf.jiterp_verify_samples = self.core_perf.jiterp_verify_samples.saturating_add(1);
        }

        if !Self::jiterp_state_matches(fast_post_state, &interp_post_state) {
            #[cfg(target_arch = "wasm32")]
            {
                self.core_perf.jiterp_verify_mismatches = self.core_perf.jiterp_verify_mismatches.saturating_add(1);
            }

            #[cfg(not(target_arch = "wasm32"))]
            panic!(
                "jiterp mismatch: op={} xo={} cia={:08X} fast_nia={:08X} interp_nia={:08X}",
                instr_raw >> 26,
                (instr_raw >> 1) & 0x3FF,
                pre_state.cia,
                fast_post_state.nia,
                interp_post_state.nia,
            );
        }

        // Continue execution using the fast-path result.
        self.jiterp_restore_state(fast_post_state);
    }

    pub(crate) fn with_scheduler(entrypoint: u32, scheduler: Scheduler<SYSTEM>) -> Self {
        System {
            vsync_pending: false,
            vi_present_seen_this_frame: false,
            execution_mode: ExecutionMode::default(),
            gekko: Gekko::new(entrypoint),
            scheduler,
            mmio: Mmio::new(),
            vi: VideoInterface::new(),
            pe: PixelEngine::new(),
            pi: ProcessorInterface::new(),
            dsp: Dsp::new(),
            exi: ExternalInterface::dummy(),
            gx: GraphicsProcessor::new(),
            cp: CommandProcessor::new(),
            di: DvdInterface::new(),
            si: SerialInterface::new(),
            ai: AudioInterface::new(),
            mi: MemoryInterface::new(),

            starlet: Starlet::new(),
            hollywood: Hollywood::new(),

            render_sink: Box::new(EmptyRenderSink::default()),
            audio_sink: Box::new(EmptyAudioSink),

            #[cfg(feature = "hooks")]
            hook_host: None,
            #[cfg(feature = "hooks")]
            hook_flags: HookFlags::empty(),
            #[cfg(feature = "hooks")]
            hook_filters: HookFilters::default(),

            #[cfg(feature = "jit")]
            jit: None,

            jiterpreter: None,
            jiterpreter_cache_dirty: false,

            #[cfg(feature = "fps-counter")]
            fps_counter: FpsCounter::new(),

            #[cfg(any(feature = "jit-stats", feature = "profile", feature = "gx-stats"))]
            heatmap: crate::profile::HeatmapConfig::default(),

            #[cfg(any(feature = "jit-stats", feature = "profile", feature = "gx-stats"))]
            vsync_count: 0,

            #[cfg(feature = "profile")]
            pprof_config: None,

            #[cfg(feature = "profile")]
            pprof_session: None,

            #[cfg(target_arch = "wasm32")]
            core_perf: CorePerfAcc::default(),
        }
    }

    #[cfg(target_arch = "wasm32")]
    #[inline(always)]
    pub fn core_perf_record_dsp_batch_ms(&mut self, ms: f64) {
        self.core_perf.dsp_batch_ms += ms * CORE_PERF_DSP_BATCH_SAMPLE_EVERY as f64;
    }

    #[cfg(target_arch = "wasm32")]
    #[inline(always)]
    pub fn core_perf_note_dsp_batch(&mut self) -> bool {
        self.core_perf.dsp_batch_calls = self.core_perf.dsp_batch_calls.saturating_add(1);
        self.core_perf
            .dsp_batch_calls
            .is_multiple_of(CORE_PERF_DSP_BATCH_SAMPLE_EVERY)
    }

    #[cfg(target_arch = "wasm32")]
    pub fn core_perf_take_window(&mut self) -> CorePerfSnapshot {
        let snapshot = CorePerfSnapshot {
            run_interp_ms: self.core_perf.run_interp_ms,
            drain_events_ms: self.core_perf.drain_events_ms,
            dsp_batch_ms: self.core_perf.dsp_batch_ms,
            step_cpu_calls: self.core_perf.step_cpu_calls,
            drain_events_calls: self.core_perf.drain_events_calls,
            dsp_batch_calls: self.core_perf.dsp_batch_calls,
            drain_budget_hits: self.core_perf.drain_budget_hits,
            jiterp_blocks: self.core_perf.jiterp_blocks,
            jiterp_instrs: self.core_perf.jiterp_instrs,
            jiterp_fast_instrs: self.core_perf.jiterp_fast_instrs,
            jiterp_fallback_dispatches: self.core_perf.jiterp_fallback_dispatches,
            jiterp_fallback_by_op: self.core_perf.jiterp_fallback_by_op,
            jiterp_fallback_op19_by_xo: self.core_perf.jiterp_fallback_op19_by_xo,
            jiterp_fallback_op31_by_xo: self.core_perf.jiterp_fallback_op31_by_xo,
            jiterp_fallback_op63_by_xo: self.core_perf.jiterp_fallback_op63_by_xo,
            jiterp_fallback_op63_xo89_by_subop: self.core_perf.jiterp_fallback_op63_xo89_by_subop,
            jiterp_cache_hits: self.core_perf.jiterp_cache_hits,
            jiterp_cache_misses: self.core_perf.jiterp_cache_misses,
            jiterp_flushes: self.core_perf.jiterp_flushes,
            jiterp_verify_samples: self.core_perf.jiterp_verify_samples,
            jiterp_verify_mismatches: self.core_perf.jiterp_verify_mismatches,
        };
        self.core_perf = CorePerfAcc::default();
        snapshot
    }

    #[inline(always)]
    pub(crate) fn jiterpreter_note_code_write(&mut self, phys: u32, len: u32) {
        if self.execution_mode != ExecutionMode::Jiterpreter || len == 0 {
            return;
        }

        let mut p = phys & crate::mmio::CODE_LINE_MASK;
        let end = phys.wrapping_add(len - 1) & crate::mmio::CODE_LINE_MASK;
        loop {
            if self.mmio.is_code_chunk(p) {
                self.jiterpreter_cache_dirty = true;
                break;
            }
            if p == end {
                break;
            }
            p = p.wrapping_add(crate::mmio::CODE_LINE_BYTES);
        }
    }

    #[inline(always)]
    fn run_cpu_pre_hook_if_enabled(&mut self, _pc: u32) {
        #[cfg(feature = "hooks")]
        if self.hook_flags.contains(HookFlags::CPU_PRE) {
            if self.hook_filters.cpu_pre.matches(_pc) {
                if let Some(mut host) = self.hook_host.take() {
                    host.on_cpu_pre(self);
                    self.sync_pending_hook_state(host.as_mut());
                    self.hook_host = Some(host);
                }
            }
        }
    }

    #[inline(always)]
    fn run_cpu_post_hook_if_enabled(&mut self, _pc: u32) {
        #[cfg(feature = "hooks")]
        if self.hook_flags.contains(HookFlags::CPU_POST) {
            if self.hook_filters.cpu_post.matches(_pc) {
                if let Some(mut host) = self.hook_host.take() {
                    host.on_cpu_post(self);
                    self.sync_pending_hook_state(host.as_mut());
                    self.hook_host = Some(host);
                }
            }
        }
    }

    #[inline(always)]
    pub(crate) fn exec_decoded_instr_raw(&mut self, cia: u32, instr_raw: u32) {
        self.run_cpu_pre_hook_if_enabled(cia);

        self.gekko.cia = cia;
        self.gekko.nia = cia.wrapping_add(4);
        #[cfg(any(all(debug_assertions, not(target_arch = "wasm32")), target_arch = "wasm32"))]
        let verify_pre_state = if self.execution_mode == ExecutionMode::Jiterpreter
            && Self::jiterp_verify_enabled()
            && self.jiterp_verify_sample_hit(instr_raw)
            && Self::jiterp_verify_candidate(instr_raw)
        {
            Some(self.jiterp_capture_state())
        } else {
            None
        };

        let handled = if self.execution_mode == ExecutionMode::Jiterpreter {
            crate::gekko::jiterp::try_execute_fast_instruction(self, instr_raw)
        } else {
            false
        };

        #[cfg(any(all(debug_assertions, not(target_arch = "wasm32")), target_arch = "wasm32"))]
        if handled {
            if let Some(pre_state) = verify_pre_state.as_ref() {
                let fast_post_state = self.jiterp_capture_state();
                self.jiterp_verify_fast_vs_interp(instr_raw, pre_state, &fast_post_state);
            }
        }

        #[cfg(target_arch = "wasm32")]
        if self.execution_mode == ExecutionMode::Jiterpreter {
            if handled {
                self.core_perf.jiterp_fast_instrs = self.core_perf.jiterp_fast_instrs.saturating_add(1);
            } else {
                self.core_perf.jiterp_fallback_dispatches =
                    self.core_perf.jiterp_fallback_dispatches.saturating_add(1);
                let op = (instr_raw >> 26) as usize;
                if op < self.core_perf.jiterp_fallback_by_op.len() {
                    self.core_perf.jiterp_fallback_by_op[op] = self.core_perf.jiterp_fallback_by_op[op].saturating_add(1);
                }
                if op == 19 {
                    let xo = ((instr_raw >> 1) & 0x3FF) as usize;
                    self.core_perf.jiterp_fallback_op19_by_xo[xo] =
                        self.core_perf.jiterp_fallback_op19_by_xo[xo].saturating_add(1);
                }
                if op == 31 {
                    let xo = ((instr_raw >> 1) & 0x3FF) as usize;
                    self.core_perf.jiterp_fallback_op31_by_xo[xo] =
                        self.core_perf.jiterp_fallback_op31_by_xo[xo].saturating_add(1);
                }
                if op == 63 {
                    let xo = ((instr_raw >> 1) & 0x3FF) as usize;
                    self.core_perf.jiterp_fallback_op63_by_xo[xo] =
                        self.core_perf.jiterp_fallback_op63_by_xo[xo].saturating_add(1);
                    if xo == 89 {
                        let subop = ((instr_raw >> 6) & 0xF) as usize;
                        self.core_perf.jiterp_fallback_op63_xo89_by_subop[subop] =
                            self.core_perf.jiterp_fallback_op63_xo89_by_subop[subop].saturating_add(1);
                    }
                }
            }
        }
        if !handled {
            let instr = crate::gekko::instruction::Instruction(instr_raw);
            crate::gekko::dispatch(self, instr);
        }
        self.scheduler.cycles += 2;

        self.run_cpu_post_hook_if_enabled(self.gekko.cia);
        self.gekko.pc = self.gekko.nia;
    }

    #[cfg(target_arch = "wasm32")]
    #[inline(always)]
    pub(crate) fn exec_jiterp_instr_raw_wasm(&mut self, cia: u32, instr_raw: u32) {
        self.run_cpu_pre_hook_if_enabled(cia);

        self.gekko.cia = cia;
        self.gekko.nia = cia.wrapping_add(4);

        let handled = crate::gekko::jiterp::try_execute_fast_instruction(self, instr_raw);

        if handled {
            self.core_perf.jiterp_fast_instrs = self.core_perf.jiterp_fast_instrs.saturating_add(1);
        } else {
            self.core_perf.jiterp_fallback_dispatches = self.core_perf.jiterp_fallback_dispatches.saturating_add(1);
            let op = (instr_raw >> 26) as usize;
            if op < self.core_perf.jiterp_fallback_by_op.len() {
                self.core_perf.jiterp_fallback_by_op[op] = self.core_perf.jiterp_fallback_by_op[op].saturating_add(1);
            }
            if op == 19 {
                let xo = ((instr_raw >> 1) & 0x3FF) as usize;
                self.core_perf.jiterp_fallback_op19_by_xo[xo] =
                    self.core_perf.jiterp_fallback_op19_by_xo[xo].saturating_add(1);
            }
            if op == 31 {
                let xo = ((instr_raw >> 1) & 0x3FF) as usize;
                self.core_perf.jiterp_fallback_op31_by_xo[xo] =
                    self.core_perf.jiterp_fallback_op31_by_xo[xo].saturating_add(1);
            }
            if op == 63 {
                let xo = ((instr_raw >> 1) & 0x3FF) as usize;
                self.core_perf.jiterp_fallback_op63_by_xo[xo] =
                    self.core_perf.jiterp_fallback_op63_by_xo[xo].saturating_add(1);
                if xo == 89 {
                    let subop = ((instr_raw >> 6) & 0xF) as usize;
                    self.core_perf.jiterp_fallback_op63_xo89_by_subop[subop] =
                        self.core_perf.jiterp_fallback_op63_xo89_by_subop[subop].saturating_add(1);
                }
            }

            let instr = crate::gekko::instruction::Instruction(instr_raw);
            crate::gekko::dispatch(self, instr);
        }

        self.scheduler.cycles += 2;

        self.run_cpu_post_hook_if_enabled(self.gekko.cia);
        self.gekko.pc = self.gekko.nia;
    }

    #[inline(always)]
    pub fn step_cpu(&mut self) {
        #[cfg(target_arch = "wasm32")]
        {
            self.core_perf.step_cpu_calls = self.core_perf.step_cpu_calls.saturating_add(1);
        }

        if self.gekko.msr.external_interrupt_enable() {
            // Deliver external interrupt when EE=1 and any enabled PI interrupt is pending
            if self.pi.interrupt_pending() {
                self.cause_external_interrupt();
                self.scheduler.cycles += 2;
                return;
            }

            if self.gekko.dec.interrupt_pending() {
                self.cause_decrementer_interrupt();
                self.scheduler.cycles += 2;
                return;
            }
        }

        // Fetch and execute next instruction
        let cia = self.gekko.pc;
        let instr_raw = {
            let top = cia >> 28;
            if top == 0x8 || top == 0xC {
                self.mmio.ram_read_u32(cia & 0x3FFF_FFFF)
            } else if SYSTEM == WII && (top == 0x9 || top == 0xD) {
                self.mmio.phys_read_u32(cia & 0x3FFF_FFFF)
            } else {
                self.mmio.fetch_instruction(cia)
            }
        };
        self.exec_decoded_instr_raw(cia, instr_raw);

    }

    /// To JIT or not to JIT, that is the question.
    pub fn set_execution_mode(&mut self, mode: ExecutionMode) {
        if self.execution_mode == ExecutionMode::Jiterpreter && mode != ExecutionMode::Jiterpreter {
            if let Some(mut jiterp) = self.jiterpreter.take() {
                jiterp.clear_cache(&mut self.mmio);
            }
            self.jiterpreter_cache_dirty = false;
        }
        self.execution_mode = mode;
        self.gx.execution_mode = mode;
    }

    /// Drain pending scheduler events, then execute one CPU instruction.
    #[inline(always)]
    pub fn step(&mut self) {
        self.drain_events();
        self.step_cpu();
    }

    pub fn run_until(&mut self, pc: u32, predicate: impl Fn(&Self) -> bool) {
        self.gekko.pc = pc;
        while !predicate(self) {
            self.step();
        }
    }

    #[inline(always)]
    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub fn prepare_frame(&mut self) {
        self.begin_frame();
        crate::flipper::si::refresh_interrupts(self);
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub fn run_until_vsync(&mut self) {
        self.prepare_frame();
        while !self.vsync_pending {
            self.scheduler.refresh_deadline();
            #[cfg(feature = "jit")]
            match self.execution_mode {
                ExecutionMode::Jit => self.run_until_deadline_jit(),
                ExecutionMode::Jiterpreter => self.run_until_deadline_jiterp(),
                ExecutionMode::Interpreter => self.run_until_deadline_interp(),
            }
            #[cfg(not(feature = "jit"))]
            if self.execution_mode == ExecutionMode::Jiterpreter {
                self.run_until_deadline_jiterp();
            } else {
                self.run_until_deadline_interp();
            }
            // Drain due events in bounded slices. Once VSync is raised we stop
            // draining to keep this frame from being consumed by post-vsync
            // event storms.
            while self.drain_events_budgeted(EVENT_DRAIN_BUDGET_PER_SLICE) {
                if self.vsync_pending {
                    break;
                }
            }
        }

        #[cfg(any(feature = "jit-stats", feature = "profile", feature = "gx-stats"))]
        self.on_vsync_boundary();
    }

    #[cfg(feature = "jit")]
    pub fn load_jit_cache(&mut self, game_id: &str) -> (usize, usize, usize, usize, usize, usize) {
        let mut ppc_compiled = 0;
        let mut ppc_skipped = 0;
        let mut dsp_compiled = 0;
        let mut dsp_skipped = 0;
        let mut vtx_compiled = 0;
        let mut vtx_skipped = 0;

        let ppc_path = crate::jit_cache::ppc_cache_path(game_id);
        if let Ok(blocks) = crate::jit_cache::load_ppc_blocks(&ppc_path) {
            tracing::info!(count = blocks.len(), "loaded PPC JIT block cache");

            if self.jit.is_none() {
                self.jit = Some(Box::new(crate::gekko::jit::JitEngine::<SYSTEM>::new()));
            }

            let mut jit = self.jit.take().unwrap();
            let (c, s) = jit.precompile_blocks(self, &blocks);
            ppc_compiled = c;
            ppc_skipped = s;

            self.jit = Some(jit);
        }

        let dsp_path = crate::jit_cache::dsp_cache_path(game_id);
        if let Ok(blocks) = crate::jit_cache::load_dsp_blocks(&dsp_path) {
            tracing::info!(count = blocks.len(), "loaded DSP JIT block cache");

            if self.dsp.jit.is_none() {
                self.dsp.jit = Some(Box::new(crate::flipper::dsp::jit::JitEngine::<SYSTEM>::new()));
            }

            let iram_ptr = self.dsp.iram.as_ptr();
            let irom_ptr = self.dsp.irom.as_ptr();
            let iram_len = self.dsp.iram.len();
            let irom_len = self.dsp.irom.len();
            let iram = unsafe { ::core::slice::from_raw_parts(iram_ptr, iram_len) };
            let irom = unsafe { ::core::slice::from_raw_parts(irom_ptr, irom_len) };

            let (c, s) = self.dsp.jit.as_mut().unwrap().precompile_blocks(iram, irom, &blocks);
            dsp_compiled = c;
            dsp_skipped = s;
        }

        let vtx_path = crate::jit_cache::vtx_cache_path(game_id);
        if let Ok(keys) = crate::jit_cache::load_vtx_keys(&vtx_path) {
            tracing::info!(count = keys.len(), "loaded vertex JIT key cache");
            let (c, s) = self.gx.jit_vtx.precompile_keys(&keys);
            vtx_compiled = c;
            vtx_skipped = s;
        }

        (
            ppc_compiled,
            ppc_skipped,
            dsp_compiled,
            dsp_skipped,
            vtx_compiled,
            vtx_skipped,
        )
    }

    #[cfg(feature = "jit")]
    pub fn save_jit_cache(&self, game_id: &str) -> std::io::Result<(usize, usize, usize)> {
        let cached_system = if SYSTEM == WII {
            crate::jit_cache::CachedSystem::Wii
        } else {
            crate::jit_cache::CachedSystem::Gc
        };

        let mut ppc_count = 0;
        let mut dsp_count = 0;

        if let Some(jit) = self.jit.as_ref() {
            let blocks = jit.cached_blocks();
            ppc_count = blocks.len();
            crate::jit_cache::save_ppc_blocks(&crate::jit_cache::ppc_cache_path(game_id), cached_system, &blocks)?;
        }

        if let Some(jit) = self.dsp.jit.as_ref() {
            let blocks = jit.cached_blocks();
            dsp_count = blocks.len();
            crate::jit_cache::save_dsp_blocks(&crate::jit_cache::dsp_cache_path(game_id), cached_system, &blocks)?;
        }

        let keys = self.gx.jit_vtx.cached_keys();
        let vtx_count = keys.len();
        crate::jit_cache::save_vtx_keys(&crate::jit_cache::vtx_cache_path(game_id), cached_system, &keys)?;

        Ok((ppc_count, dsp_count, vtx_count))
    }

    #[cfg(any(feature = "jit-stats", feature = "profile", feature = "gx-stats"))]
    fn on_vsync_boundary(&mut self) {
        self.vsync_count = self.vsync_count.wrapping_add(1);

        #[cfg(feature = "jit-stats")]
        self.dump_heatmap_if_due();

        #[cfg(feature = "gx-stats")]
        self.dump_gx_stats_if_due();

        #[cfg(feature = "profile")]
        self.tick_pprof_session();
    }

    #[cfg(feature = "gx-stats")]
    fn dump_gx_stats_if_due(&self) {
        if !self.heatmap.enabled || self.heatmap.interval_frames == 0 {
            return;
        }

        if self.vsync_count % self.heatmap.interval_frames as u64 != 0 {
            return;
        }

        use std::io::Write;

        let path = self.heatmap.out_dir.join("gx-stats.txt");
        let s = &self.gx.stats;
        let avg_draw_ns = if s.draw_calls > 0 {
            s.create_draw_call_ns / s.draw_calls
        } else {
            0
        };

        let actions_sent: u64 = 0;
        let channel_len: usize = 0;
        let channel_cap: usize = 0;
        let result = crate::profile::write_file_atomic(&path, |f| {
            writeln!(
                f,
                "vsync_count={}\ndraw_calls={}\nvertices={}\nfifo_bytes={}\ntexture_loads={}\nxfb_presents={}\nbp_writes={}\nxf_writes={}\ncreate_draw_call_ns={}\navg_draw_call_ns={}\nrender_actions_sent={}\nrender_channel_len={}\nrender_channel_cap={}",
                self.vsync_count,
                s.draw_calls,
                s.vertices,
                s.fifo_bytes,
                s.texture_loads,
                s.xfb_presents,
                s.bp_writes,
                s.xf_writes,
                s.create_draw_call_ns,
                avg_draw_ns,
                actions_sent,
                channel_len,
                channel_cap,
            )?;
            writeln!(f, "\n--- draws by primitive ---")?;

            use crate::flipper::gx::draw::Primitive;

            const VARIANTS: [Primitive; 7] = [
                Primitive::Quads,
                Primitive::Triangles,
                Primitive::TriangleStrip,
                Primitive::TriangleFan,
                Primitive::Lines,
                Primitive::LineStrip,
                Primitive::Points,
            ];

            for p in VARIANTS {
                let count = s.draws_by_primitive[(p as usize) & 0x7];
                writeln!(f, "  {:>16}  {:?}", count, p)?;
            }

            Ok(())
        });

        if let Err(err) = result {
            tracing::warn!(?err, "gx-stats sidecar write failed");
        }
    }

    #[cfg(feature = "jit-stats")]
    fn dump_heatmap_if_due(&mut self) {
        if !self.heatmap.enabled || self.heatmap.interval_frames == 0 {
            return;
        }

        if self.vsync_count % self.heatmap.interval_frames as u64 != 0 {
            return;
        }

        #[cfg(feature = "jit")]
        if let Some(jit) = self.jit.as_ref() {
            if let Err(err) = jit.dump_hot_blocks_csv(self.heatmap.top_k, &self.heatmap.ppc_csv_path()) {
                tracing::warn!(?err, "ppc heatmap dump failed");
            }
        }

        if let Some(jit) = self.dsp.jit.as_ref() {
            if let Err(err) = jit.dump_hot_blocks_csv(self.heatmap.top_k, &self.heatmap.dsp_csv_path()) {
                tracing::warn!(?err, "dsp heatmap dump failed");
            }
        }

        #[cfg(feature = "jit")]
        self.dump_idle_skip_sidecar();
    }

    #[cfg(all(feature = "jit-stats", feature = "jit"))]
    fn dump_idle_skip_sidecar(&self) {
        use std::io::Write;
        use std::sync::atomic::Ordering;

        let calls = crate::gekko::jit::runtime::IDLE_SKIP_CALLS.load(Ordering::Relaxed);
        let cycles = crate::gekko::jit::runtime::IDLE_SKIP_CYCLES_ADVANCED.load(Ordering::Relaxed);
        let avg = if calls > 0 { cycles as f64 / calls as f64 } else { 0.0 };
        let dsp_suspends = crate::flipper::dsp::DSP_SUSPEND_COUNT.load(Ordering::Relaxed);
        let dsp_wakes = crate::flipper::dsp::DSP_WAKE_COUNT.load(Ordering::Relaxed);

        let event_breakdown = self.event_breakdown_top_n(20);

        let path = self.heatmap.out_dir.join("idle-skip.txt");
        let result = crate::profile::write_file_atomic(&path, |f| {
            writeln!(
                f,
                "vsync_count={}\nppc_idle_calls={}\nppc_cycles_advanced={}\nppc_avg_advance={:.1}\ndsp_suspends={}\ndsp_wakes={}",
                self.vsync_count, calls, cycles, avg, dsp_suspends, dsp_wakes
            )?;

            writeln!(f, "\n--- top scheduler events by fire count ---")?;

            for (name, count) in &event_breakdown {
                writeln!(f, "{:>10}  {}", count, name)?;
            }

            Ok(())
        });
        if let Err(err) = result {
            tracing::warn!(?err, "idle-skip sidecar write failed");
        }
    }

    #[cfg(all(feature = "jit-stats", feature = "jit"))]
    fn event_breakdown_top_n(&self, n: usize) -> Vec<(String, u64)> {
        let mut entries: Vec<(String, u64)> = self
            .scheduler
            .event_fire_counts
            .iter()
            .map(|(&addr, &count)| (Self::resolve_handler_name(addr), count))
            .collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        entries.truncate(n);
        entries
    }

    #[cfg(all(feature = "jit-stats", feature = "jit"))]
    fn resolve_handler_name(addr: usize) -> String {
        crate::profile::resolve_symbol(addr).unwrap_or_else(|| format!("<unresolved {:#018x}>", addr))
    }

    #[cfg(feature = "profile")]
    fn tick_pprof_session(&mut self) {
        if self.pprof_session.is_none() {
            if let Some(cfg) = self.pprof_config.as_ref() {
                if self.vsync_count >= cfg.delay_vsyncs as u64 {
                    let cfg = self.pprof_config.take().unwrap();
                    match crate::profile::IpSampler::start_for_current_thread(cfg.hz, cfg.secs, cfg.out.clone()) {
                        Ok(s) => {
                            tracing::info!(
                                hz = cfg.hz,
                                secs = cfg.secs,
                                out = %cfg.out.display(),
                                "pprof: sampling started",
                            );
                            self.pprof_session = Some(s);
                        }
                        Err(err) => tracing::warn!(?err, "failed to start pprof sampler"),
                    }
                }
            }
        }

        let expired = self.pprof_session.as_ref().is_some_and(|s| s.expired());
        if expired {
            let session = self.pprof_session.take().unwrap();
            match session.finish() {
                Ok(path) => tracing::info!("pprof samples written to {}", path.display()),
                Err(err) => tracing::warn!(?err, "pprof sample dump failed"),
            }
        }
    }

    #[inline(always)]
    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    fn run_until_deadline_interp(&mut self) {
        #[cfg(target_arch = "wasm32")]
        let perf_start = Instant::now();

        while self.scheduler.cycles < self.scheduler.next_deadline() {
            self.step_cpu();
        }

        #[cfg(target_arch = "wasm32")]
        {
            self.core_perf.run_interp_ms += perf_start.elapsed().as_secs_f64() * 1000.0;
        }
    }

    #[inline(always)]
    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    fn run_until_deadline_jiterp(&mut self) {
        #[cfg(target_arch = "wasm32")]
        const JITERP_IRQ_CHECK_EVERY_BLOCKS: u32 = 8;
        #[cfg(not(target_arch = "wasm32"))]
        const JITERP_IRQ_CHECK_EVERY_BLOCKS: u32 = 32;

        #[cfg(target_arch = "wasm32")]
        let perf_start = Instant::now();

        let mut jiterp = self.jiterpreter.take().unwrap_or_else(|| Box::new(crate::gekko::jiterp::JiterpEngine::<SYSTEM>::new()));
        let mut blocks_until_irq_check: u32 = 0;

        while self.scheduler.cycles < self.scheduler.next_deadline() {
            if blocks_until_irq_check == 0 && self.gekko.msr.external_interrupt_enable() {
                if self.pi.interrupt_pending() {
                    self.cause_external_interrupt();
                    self.scheduler.cycles += 2;
                    blocks_until_irq_check = JITERP_IRQ_CHECK_EVERY_BLOCKS;
                    continue;
                }

                if self.gekko.dec.interrupt_pending() {
                    self.cause_decrementer_interrupt();
                    self.scheduler.cycles += 2;
                    blocks_until_irq_check = JITERP_IRQ_CHECK_EVERY_BLOCKS;
                    continue;
                }

                blocks_until_irq_check = JITERP_IRQ_CHECK_EVERY_BLOCKS;
            } else if blocks_until_irq_check > 0 {
                blocks_until_irq_check -= 1;
            }

            if self.jiterpreter_cache_dirty {
                jiterp.clear_cache(&mut self.mmio);
                self.jiterpreter_cache_dirty = false;
                #[cfg(target_arch = "wasm32")]
                {
                    self.core_perf.jiterp_flushes = self.core_perf.jiterp_flushes.saturating_add(1);
                }
            }

            let stats = jiterp.run_block(self);
            if stats.instrs == 0 {
                blocks_until_irq_check = 0;
                self.step_cpu();
                continue;
            }

            #[cfg(target_arch = "wasm32")]
            {
                self.core_perf.jiterp_blocks = self.core_perf.jiterp_blocks.saturating_add(stats.blocks as u64);
                self.core_perf.jiterp_instrs = self.core_perf.jiterp_instrs.saturating_add(stats.instrs as u64);
                self.core_perf.jiterp_cache_hits = self
                    .core_perf
                    .jiterp_cache_hits
                    .saturating_add(stats.cache_hits as u64);
                self.core_perf.jiterp_cache_misses = self
                    .core_perf
                    .jiterp_cache_misses
                    .saturating_add(stats.cache_misses as u64);
            }
        }

        self.jiterpreter = Some(jiterp);

        #[cfg(target_arch = "wasm32")]
        {
            self.core_perf.run_interp_ms += perf_start.elapsed().as_secs_f64() * 1000.0;
        }
    }

    /// JIT inner loop: runs compiled blocks back-to-back until
    /// `scheduler.cycles >= next_deadline`. Interrupts are checked at block
    /// boundaries.
    #[cfg(feature = "jit")]
    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    fn run_until_deadline_jit(&mut self) {
        let mut jit = match self.jit.take() {
            Some(jit) => jit,
            None => Box::new(crate::gekko::jit::JitEngine::<SYSTEM>::new()),
        };

        while self.scheduler.cycles < self.scheduler.next_deadline() {
            if self.gekko.msr.external_interrupt_enable() {
                if self.pi.interrupt_pending() {
                    self.cause_external_interrupt();
                    self.scheduler.cycles += 2;
                    continue;
                }

                if self.gekko.dec.interrupt_pending() {
                    self.cause_decrementer_interrupt();
                    self.scheduler.cycles += 2;
                    continue;
                }
            }

            jit.run_block(self);

            if self.mmio.jit_dirty != 0 {
                jit.drain_scratch.extend(self.mmio.pending_icbi.drain());
                while let Some(line) = jit.drain_scratch.pop() {
                    jit.invalidate_line(&mut self.mmio, line);
                }
                self.mmio.jit_dirty = 0;
            }
        }

        self.jit = Some(jit);
    }

    pub fn frame_size(&self) -> (usize, usize) {
        let fmt = self.vi.dcr.video_format();
        (fmt.columns(), fmt.lines())
    }

    pub fn apply_host_input(&mut self, input: &HostInput) {
        match input {
            HostInput::Gc(pad) if SYSTEM == GC => {
                self.si.pad_state[0] = *pad;
            }
            HostInput::Wii {
                wiimote_buttons,
                wiimote_shake,
                nunchuk_buttons,
                nunchuk_stick_x,
                nunchuk_stick_y,
                ir_pointer,
            } if SYSTEM == WII => {
                self.starlet.set_wiimote_buttons(*wiimote_buttons);
                self.starlet.set_wiimote_shake(*wiimote_shake);
                self.starlet
                    .set_nunchuk(*nunchuk_buttons, *nunchuk_stick_x, *nunchuk_stick_y);
                self.starlet.set_ir_pointer(*ir_pointer);
            }
            _ => unreachable!("invalid host input for system"),
        }
    }

    #[inline(always)]
    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub fn begin_frame(&mut self) {
        self.vsync_pending = false;
        self.si.update_polling();
    }

    #[inline(always)]
    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub fn drain_events(&mut self) {
        #[cfg(target_arch = "wasm32")]
        let perf_start = {
            self.core_perf.drain_events_calls = self.core_perf.drain_events_calls.saturating_add(1);
            if self
                .core_perf
                .drain_events_calls
                .is_multiple_of(CORE_PERF_DRAIN_EVENTS_SAMPLE_EVERY)
            {
                Some(Instant::now())
            } else {
                None
            }
        };

        while let Some(f) = self.scheduler.take_due_event() {
            f(self);
        }

        self.scheduler.refresh_deadline();

        #[cfg(target_arch = "wasm32")]
        {
            if let Some(start) = perf_start {
                self.core_perf.drain_events_ms +=
                    start.elapsed().as_secs_f64() * 1000.0 * CORE_PERF_DRAIN_EVENTS_SAMPLE_EVERY as f64;
            }
        }
    }

    #[inline(always)]
    fn drain_events_budgeted(&mut self, max_events: usize) -> bool {
        #[cfg(target_arch = "wasm32")]
        let perf_start = {
            self.core_perf.drain_events_calls = self.core_perf.drain_events_calls.saturating_add(1);
            if self
                .core_perf
                .drain_events_calls
                .is_multiple_of(CORE_PERF_DRAIN_EVENTS_SAMPLE_EVERY)
            {
                Some(Instant::now())
            } else {
                None
            }
        };

        let mut count = 0usize;
        while count < max_events {
            let Some(f) = self.scheduler.take_due_event() else {
                break;
            };
            f(self);
            count += 1;
            if self.vsync_pending {
                break;
            }
        }

        let backlog = self.scheduler.has_due_event();
        self.scheduler.refresh_deadline();

        #[cfg(target_arch = "wasm32")]
        {
            if count == max_events && backlog {
                self.core_perf.drain_budget_hits = self.core_perf.drain_budget_hits.saturating_add(1);
            }

            if let Some(start) = perf_start {
                self.core_perf.drain_events_ms +=
                    start.elapsed().as_secs_f64() * 1000.0 * CORE_PERF_DRAIN_EVENTS_SAMPLE_EVERY as f64;
            }
        }

        backlog && !self.vsync_pending
    }

    pub fn load_image(&mut self, exe: &impl Executable) {
        let data = exe.data();

        // Copy TEXT sections to memory
        for section in exe.text_sections() {
            for i in 0..section.size {
                let addr = section.vaddr + i;
                let value = data[(section.offset + i) as usize];
                self.mmio.virt_write_u8(addr, value);
            }
        }

        // Copy DATA sections to memory
        for section in exe.data_sections() {
            for i in 0..section.size {
                let addr = section.vaddr + i;
                let value = data[(section.offset + i) as usize];
                self.mmio.virt_write_u8(addr, value);
            }
        }

        // Zero out the BSS section
        let (bss_start, bss_size) = exe.bss();
        for i in 0..bss_size {
            let addr = bss_start + i;
            self.mmio.virt_write_u8(addr, 0);
        }
    }

    #[cfg(feature = "hooks")]
    #[inline(always)]
    pub fn apply_hook_state(&mut self, state: HookState) {
        self.hook_flags = state.flags;
        self.hook_filters = state.filters;
    }

    #[cfg(feature = "hooks")]
    #[inline(always)]
    pub fn sync_pending_hook_state(&mut self, host: &mut dyn Host<SYSTEM>) {
        #[cfg(feature = "hooks-mut-traps")]
        match host.take_pending_hook_state() {
            Ok(Some(state)) => self.apply_hook_state(state),
            Ok(None) => {}
            Err(err) => tracing::error!(target: "script", error = %err, "failed to refresh script traps"),
        }

        #[cfg(not(feature = "hooks-mut-traps"))]
        let _ = host;
    }

    #[cfg(feature = "hooks")]
    pub fn set_hook_host(&mut self, host: Box<dyn Host<SYSTEM> + Send>) {
        self.apply_hook_state(host.hook_state());
        self.hook_host = Some(host);
    }

    #[cfg(feature = "hooks")]
    pub fn has_hook_host(&self) -> bool {
        self.hook_host.is_some()
    }

    #[cfg(not(feature = "hooks"))]
    pub fn has_hook_host(&self) -> bool {
        false
    }

    #[cfg(all(feature = "hooks", feature = "hooks-mut-traps"))]
    pub fn refresh_hook_traps(&mut self) -> Result<(), String> {
        let Some(mut host) = self.hook_host.take() else {
            return Ok(());
        };

        let refresh_result = host.force_refresh_traps();
        match refresh_result {
            Ok(state) => {
                self.apply_hook_state(state);
                self.hook_host = Some(host);
                Ok(())
            }
            Err(err) => {
                self.hook_host = Some(host);
                Err(err)
            }
        }
    }
}
