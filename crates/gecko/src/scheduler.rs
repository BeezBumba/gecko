use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::collections::binary_heap::PeekMut;

use crate::gamecube::GameCube;

pub const CYCLES_PER_VSYNC: u64 = 486_000_000 / 60; // TODO: fix
pub const TIMEBASE_DIVISOR: u64 = 12;
pub const CPU_CYCLES_PER_DSP_TICK: u64 = 6; // ~486MHz CPU / ~81MHz DSP
pub const DSP_BATCH_SIZE: u64 = 1024;

pub type ScheduledFn = fn(&mut GameCube);

#[derive(Clone, Copy, Eq)]
struct ScheduledEvent {
    deadline: u64,
    f: ScheduledFn,
}

impl PartialEq for ScheduledEvent {
    fn eq(&self, other: &Self) -> bool {
        self.deadline == other.deadline
    }
}
impl Ord for ScheduledEvent {
    fn cmp(&self, other: &Self) -> Ordering {
        other.deadline.cmp(&self.deadline)
    }
}
impl PartialOrd for ScheduledEvent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub struct Scheduler {
    pub cycles: u64,
    next_deadline: u64,
    timebase_offset: i64,
    events: BinaryHeap<ScheduledEvent>,
}

impl Scheduler {
    pub fn new() -> Self {
        let mut s = Scheduler {
            cycles: 0,
            next_deadline: 0,
            timebase_offset: 0,
            events: BinaryHeap::new(),
        };
        s.schedule_at(CYCLES_PER_VSYNC, crate::scheduler::vsync_handler);
        s.schedule_at(
            CPU_CYCLES_PER_DSP_TICK * DSP_BATCH_SIZE,
            crate::scheduler::dsp_batch_handler,
        );
        s
    }

    /// Set `next_deadline` to the next event deadline so the CPU knows
    /// how far it can run before an event must be serviced.
    /// This may later be updated if an event is scheduled sooner than the current target.
    #[inline(always)]
    pub fn update_deadline(&mut self) {
        self.next_deadline = self.events.peek().map_or(self.cycles, |e| e.deadline);
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

    pub fn schedule_at(&mut self, deadline: u64, f: ScheduledFn) {
        self.events.push(ScheduledEvent { deadline, f });
        if deadline < self.next_deadline {
            self.next_deadline = deadline;
        }
    }

    pub fn schedule_in(&mut self, delay: u64, f: ScheduledFn) {
        let deadline = self.cycles + delay;
        self.schedule_at(deadline, f);
    }

    pub fn poll(&mut self) -> Option<ScheduledFn> {
        let top = self.events.peek_mut()?;
        if self.cycles >= top.deadline {
            Some(PeekMut::pop(top).f)
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn next_deadline(&self) -> u64 {
        self.next_deadline
    }
}

/// Reschedules itself every frame.
pub fn vsync_handler(gc: &mut GameCube) {
    gc.vsync_pending = true;
    gc.scheduler
        .schedule_in(CYCLES_PER_VSYNC, crate::scheduler::vsync_handler);
}

/// Reschedules itself every DSP batch.
pub fn dsp_batch_handler(gc: &mut GameCube) {
    gc.execute_dsp_batch();
    gc.scheduler.schedule_in(
        crate::scheduler::CPU_CYCLES_PER_DSP_TICK * crate::scheduler::DSP_BATCH_SIZE,
        crate::scheduler::dsp_batch_handler,
    );
}
