pub mod regs;

use crate::mmio::constants::AI_BASE;
use crate::mmio::traits::{MmioAccess, MmioRegister};

/// CPU cycles per AI sample at 32kHz: 486MHz / 32000 = 15187
const CYCLES_PER_SAMPLE_32K: u64 = 15187;
/// CPU cycles per AI sample at 48kHz: 486MHz / 48000 = 10125
const CYCLES_PER_SAMPLE_48K: u64 = 10125;

pub struct Ai {
    pub control: regs::AiControl,
    pub volume: regs::AiVolume,
    pub sample_counter: regs::AiSampleCounter,
    pub interrupt_timing: regs::AiInterruptTiming,
    pub sample_counter_base_cycle: u64,
    pub sample_counter_reset_pending: bool,
}

impl Ai {
    pub fn new() -> Self {
        Self {
            control: regs::AiControl::from_raw(0),
            volume: regs::AiVolume::from_raw(0),
            sample_counter: regs::AiSampleCounter::from_raw(0),
            interrupt_timing: regs::AiInterruptTiming::from_raw(0),
            sample_counter_base_cycle: 0,
            sample_counter_reset_pending: false,
        }
    }

    pub fn interrupt_active(&self) -> bool {
        self.control.interrupt() && self.control.interrupt_mask()
    }

    /// Compute the current sample counter based on elapsed cycles
    pub fn sample_count(&self, current_cycles: u64) -> u32 {
        if self.control.playback_status() != regs::Status::Play {
            return 0;
        }

        let elapsed = current_cycles.saturating_sub(self.sample_counter_base_cycle);
        let cycles_per_sample = match self.control.sample_rate() {
            regs::SampleRate::Rate32KHz => CYCLES_PER_SAMPLE_32K,
            regs::SampleRate::Rate48KHz => CYCLES_PER_SAMPLE_48K,
        };
        (elapsed / cycles_per_sample) as u32
    }

    /// Check if the sample counter has reached the interrupt timing threshold
    pub fn check_sample_counter_interrupt(&mut self, current_cycles: u64) {
        let threshold = self.interrupt_timing.sample_count();
        if threshold == 0 {
            return;
        }

        let count = self.sample_count(current_cycles);
        self.sample_counter = regs::AiSampleCounter::from_raw(count);

        if count >= threshold {
            self.control = self.control.with_interrupt(true);
        }
    }

    crate::impl_mmio_dispatch!(
        regs::AiControl,
        regs::AiVolume,
        regs::AiInterruptTiming,
        regs::AiSampleCounter,
    );

    pub fn mmio_read_u8(&mut self, offset: u32) -> u8 {
        self.read_raw(AI_BASE + offset, 1).unwrap_or_else(|| {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled AI read_u8");
            0
        }) as u8
    }

    pub fn mmio_write_u8(&mut self, offset: u32, val: u8) {
        if !self.write_raw(AI_BASE + offset, 1, val as u32) {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled AI write_u8");
        }
    }

    pub fn mmio_read_u16(&mut self, offset: u32) -> u16 {
        self.read_raw(AI_BASE + offset, 2).unwrap_or_else(|| {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled AI read_u16");
            0
        }) as u16
    }

    pub fn mmio_write_u16(&mut self, offset: u32, val: u16) {
        if !self.write_raw(AI_BASE + offset, 2, val as u32) {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled AI write_u16");
        }
    }

    pub fn mmio_read_u32(&mut self, offset: u32) -> u32 {
        self.read_raw(AI_BASE + offset, 4).unwrap_or_else(|| {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled AI read_u32");
            0
        })
    }

    pub fn mmio_write_u32(&mut self, offset: u32, val: u32) {
        if !self.write_raw(AI_BASE + offset, 4, val) {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled AI write_u32");
        }
    }
}

impl crate::gekko::Gekko {
    pub fn check_ai_interrupts(&mut self) {
        self.ai.check_sample_counter_interrupt(self.scheduler.cycles);
        if self.ai.interrupt_active() {
            self.pi.assert_interrupt(crate::flipper::pi::InterruptFlag::Ai);
        } else {
            self.pi.clear_interrupt(crate::flipper::pi::InterruptFlag::Ai);
        }
    }

    pub fn check_sample_counter_reset(&mut self) {
        if self.ai.sample_counter_reset_pending {
            self.ai.sample_counter_base_cycle = self.scheduler.cycles;
            self.ai.sample_counter_reset_pending = false;
        }
    }
}
