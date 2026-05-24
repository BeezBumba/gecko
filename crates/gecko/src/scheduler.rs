use std::collections::VecDeque;

use crate::flipper::vi::regs::RefreshRate;
use crate::system::{self, System, SystemId};

pub const TIMEBASE_DIVISOR: u64 = 12;
#[cfg(target_arch = "wasm32")]
pub const DSP_BATCH_SIZE: u64 = 2048;
#[cfg(not(target_arch = "wasm32"))]
pub const DSP_BATCH_SIZE: u64 = 1024;

#[inline(always)]
#[rustfmt::skip]
pub const fn cpu_clock(system: SystemId) -> u64 {
    match system {
        system::WII => 729_000_000,
        system::GC  => 486_000_000,
        _ => unreachable!(),
    }
}

#[inline(always)]
#[rustfmt::skip]
pub const fn cpu_cycles_per_dsp_tick(system: SystemId) -> u64 {
    match system {
        system::WII => 9, // 729 MHz / 81 MHz
        system::GC  => 6, // 486 MHz / 81 MHz
        _ => unreachable!(),
    }
}

#[inline(always)]
pub const fn microseconds_to_cycles(system: SystemId, us: u64) -> u64 {
    us * (self::cpu_clock(system) / 1_000_000)
}

pub type ScheduledFn<const SYSTEM: SystemId> = fn(&mut System<SYSTEM>);

#[derive(Clone, Copy)]
struct ScheduledEvent<const SYSTEM: SystemId> {
    deadline: u64,
    f: ScheduledFn<SYSTEM>,
}

pub struct Scheduler<const SYSTEM: SystemId> {
    pub cycles: u64,
    pub(crate) next_deadline: u64,
    pub(crate) timebase_offset: i64,
    events: VecDeque<ScheduledEvent<SYSTEM>>,
    #[cfg(feature = "jit-stats")]
    pub(crate) event_fire_counts: rustc_hash::FxHashMap<usize, u64>,
}

impl<const SYSTEM: SystemId> Scheduler<SYSTEM> {
    pub fn empty() -> Self {
        Scheduler {
            cycles: 0,
            next_deadline: u64::MAX,
            timebase_offset: 0,
            events: VecDeque::with_capacity(8),
            #[cfg(feature = "jit-stats")]
            event_fire_counts: rustc_hash::FxHashMap::default(),
        }
    }

    #[inline(always)]
    pub fn refresh_deadline(&mut self) {
        self.next_deadline = self.events.front().map_or(u64::MAX, |e| e.deadline);
    }

    pub fn timebase(&self) -> u64 {
        ((self.cycles / TIMEBASE_DIVISOR) as i64 + self.timebase_offset) as u64
    }

    pub fn set_timebase_lower(&mut self, val: u32) {
        let current = self.timebase();
        let new_tb = (current & 0xFFFF_FFFF_0000_0000) | val as u64;
        self.timebase_offset = new_tb as i64 - (self.cycles / TIMEBASE_DIVISOR) as i64;
    }

    pub fn set_timebase_upper(&mut self, val: u32) {
        let current = self.timebase();
        let new_tb = ((val as u64) << 32) | (current & 0xFFFF_FFFF);
        self.timebase_offset = new_tb as i64 - (self.cycles / TIMEBASE_DIVISOR) as i64;
    }

    pub fn timebase_lower(&self) -> u32 {
        self.timebase() as u32
    }

    pub fn timebase_upper(&self) -> u32 {
        (self.timebase() >> 32) as u32
    }

    /// Insert an event keeping the deque sorted by deadline (earliest first).
    pub fn schedule_at(&mut self, deadline: u64, f: ScheduledFn<SYSTEM>) {
        if self.events.back().is_none_or(|e| e.deadline <= deadline) {
            self.events.push_back(ScheduledEvent { deadline, f });
        } else {
            let pos = self.events.partition_point(|e| e.deadline <= deadline);
            self.events.insert(pos, ScheduledEvent { deadline, f });
        }
        self.next_deadline = self.next_deadline.min(deadline);
    }

    pub fn cancel(&mut self, f: ScheduledFn<SYSTEM>) {
        self.events.retain(|e| !std::ptr::fn_addr_eq(e.f, f));
        self.refresh_deadline();
    }

    pub fn schedule_in(&mut self, delay: u64, f: ScheduledFn<SYSTEM>) {
        let deadline = self.cycles + delay;
        self.schedule_at(deadline, f);
    }

    #[inline(always)]
    pub fn poll(&mut self) -> Option<ScheduledFn<SYSTEM>> {
        let front = self.events.front()?;
        if self.cycles >= front.deadline {
            let f = self.events.pop_front().unwrap().f;
            self.refresh_deadline();
            #[cfg(feature = "jit-stats")]
            {
                *self.event_fire_counts.entry(f as *const () as usize).or_insert(0) += 1;
            }
            Some(f)
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn take_due_event(&mut self) -> Option<ScheduledFn<SYSTEM>> {
        let front = self.events.front()?;
        if self.cycles >= front.deadline {
            let f = self.events.pop_front().unwrap().f;
            #[cfg(feature = "jit-stats")]
            {
                *self.event_fire_counts.entry(f as *const () as usize).or_insert(0) += 1;
            }
            Some(f)
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn has_due_event(&self) -> bool {
        self.events
            .front()
            .is_some_and(|event| self.cycles >= event.deadline)
    }

    #[inline(always)]
    pub fn next_deadline(&self) -> u64 {
        self.next_deadline
    }
}

impl Scheduler<{ crate::system::GC }> {
    pub fn new_gamecube() -> Self {
        Self::with_default_events()
    }
}

impl Scheduler<{ crate::system::WII }> {
    pub fn new_wii() -> Self {
        Self::with_default_events()
    }
}

impl<const SYSTEM: SystemId> Scheduler<SYSTEM> {
    fn with_default_events() -> Self {
        let mut s = Self::empty();
        let initial_refresh_rate = RefreshRate::Hz60; // TODO: Detect IPL and schedule accordingly
        s.schedule_at(
            initial_refresh_rate.cycles_per_frame(SYSTEM),
            self::vsync_handler::<SYSTEM>,
        );
        s.schedule_at(
            crate::gekko::dec::cycles_until_underflow(u32::MAX),
            crate::gekko::dec::underflow_handler::<SYSTEM>,
        );
        s.schedule_at(
            crate::flipper::cp::PUMP_INTERVAL_CYCLES,
            crate::flipper::cp::pump_handler::<SYSTEM>,
        );
        #[cfg(feature = "fps-counter")]
        s.schedule_at(self::cpu_clock(SYSTEM), crate::fps::fps_handler::<SYSTEM>);
        s
    }
}

/// Reschedules itself every frame.
pub fn vsync_handler<const SYSTEM: SystemId>(sys: &mut System<SYSTEM>) {
    if !sys.vi_present_seen_this_frame {
        sys.vsync_pending = true;

        #[cfg(feature = "fps-counter")]
        {
            sys.fps_counter.vsync_count += 1;
        }
    }

    sys.vi_present_seen_this_frame = false;
    let rate = sys.vi.dcr.video_format().refresh_rate();
    sys.scheduler
        .schedule_in(rate.cycles_per_frame(SYSTEM), self::vsync_handler::<SYSTEM>);
}

#[inline(always)]
pub const fn dsp_batch_interval(system: SystemId) -> u64 {
    self::cpu_cycles_per_dsp_tick(system) * self::DSP_BATCH_SIZE
}

#[inline(always)]
pub const fn dsp_batch_interval_steps(system: SystemId, steps: u64) -> u64 {
    self::cpu_cycles_per_dsp_tick(system) * steps
}

pub fn dsp_batch_handler<const SYSTEM: SystemId>(sys: &mut System<SYSTEM>) {
    sys.dsp.scheduler_event_pending = false;

    #[cfg(target_arch = "wasm32")]
    let perf_start = sys.core_perf_note_dsp_batch().then(web_time::Instant::now);

    if sys.dsp_should_suspend_scheduler() {
        sys.dsp.scheduler_suspended = true;
        sys.dsp.batch_pressure_streak = 0;
        #[cfg(feature = "jit-stats")]
        crate::flipper::dsp::DSP_SUSPEND_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        return;
    }

    sys.execute_dsp_batch();

    #[cfg(target_arch = "wasm32")]
    {
        if let Some(start) = perf_start {
            sys.core_perf_record_dsp_batch_ms(start.elapsed().as_secs_f64() * 1000.0);
        }
    }

    if sys.dsp.csr.halt() || sys.dsp.csr.reset() {
        return;
    }

    if sys.dsp_should_suspend_scheduler() {
        sys.dsp.scheduler_suspended = true;
        sys.dsp.batch_pressure_streak = 0;
        #[cfg(feature = "jit-stats")]
        crate::flipper::dsp::DSP_SUSPEND_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        return;
    }

    sys.tune_dsp_batch_steps();

    crate::flipper::dsp::schedule_dsp_batch::<SYSTEM>(sys);
}
