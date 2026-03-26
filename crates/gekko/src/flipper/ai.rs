pub mod regs;

use crate::mmio::constants::AI_BASE;
use crate::mmio::traits::{MmioAccess, MmioRegister, MmioRw};

/// CPU cycles per AI sample at 32kHz: 486MHz / 32000 = 15187
const CYCLES_PER_SAMPLE_32K: u64 = 15187;
/// CPU cycles per AI sample at 48kHz: 486MHz / 48000 = 10125
const CYCLES_PER_SAMPLE_48K: u64 = 10125;

pub struct AudioInterface {
    pub control: regs::AiControl,
    pub volume: regs::AiVolume,
    pub sample_counter: regs::AiSampleCounter,
    pub interrupt_timing: regs::AiInterruptTiming,
    pub sample_counter_base_cycle: u64,
    pub sample_counter_reset_pending: bool,
}

impl AudioInterface {
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

}

impl MmioRw for AudioInterface {
    const BASE: u32 = AI_BASE;
    const NAME: &'static str = "AI";

    crate::impl_mmio_dispatch!(
        regs::AiControl,
        regs::AiVolume,
        regs::AiInterruptTiming,
        regs::AiSampleCounter,
    );
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
