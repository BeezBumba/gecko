/// Maximum backward branch distance (in bytes) considered a potential polling loop
const IDLE_LOOP_MAX_SIZE: u32 = 10 * 4;
/// Number of consecutive loop iterations before we skip to the next event
const IDLE_LOOP_THRESHOLD: u32 = 10;

pub struct IdleDetector {
    enabled: bool,
    loop_pc: Option<u32>,
    loop_end: u32,
    loop_count: u32,
}

impl IdleDetector {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            loop_pc: None,
            loop_end: 0,
            loop_count: 0,
        }
    }

    #[inline]
    pub fn check(&mut self, cia: u32, nia: u32) -> bool {
        if !self.enabled {
            return false;
        }

        if nia == cia {
            // Skip branch to self
            tracing::debug!("detected branch to self at {:08X}", cia);
            return true;
        }

        if nia < cia && cia.wrapping_sub(nia) <= IDLE_LOOP_MAX_SIZE {
            // Potential polling loop
            return self.track_backward_branch(nia, cia);
        }

        // We moved outside of the tracked loop, reset
        self.try_reset(cia);
        false
    }

    fn track_backward_branch(&mut self, target: u32, branch_pc: u32) -> bool {
        if self.loop_pc == Some(target) {
            self.loop_count += 1;
            if self.loop_count >= IDLE_LOOP_THRESHOLD {
                self.loop_count = 0;
                tracing::debug!("detected idle loop at {:08X} (branched from {:08X})", target, branch_pc);
                return true;
            }
        } else {
            self.loop_pc = Some(target);
            self.loop_end = branch_pc;
            self.loop_count = 1;
        }

        false
    }

    fn try_reset(&mut self, cia: u32) {
        if let Some(start) = self.loop_pc {
            if cia < start || cia > self.loop_end {
                self.loop_pc = None;
                self.loop_count = 0;
            }
        }
    }
}
