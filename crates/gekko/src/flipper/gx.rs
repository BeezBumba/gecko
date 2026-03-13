pub mod constants;
pub mod regs;

use super::pi::InterruptFlag;
use crate::{
    flipper::gx::constants::{BP_REG_SIZE, CP_REG_SIZE, XF_MEM_SIZE},
    gekko::Gekko,
};

pub struct Gx {
    pub raise_interrupt: bool,
    bp_regs: Vec<u32>,
    cp_regs: Vec<u32>,
    xf_mem: Vec<u32>,
    fifo: Vec<u8>,
}

impl Gx {
    pub fn new() -> Self {
        Gx {
            raise_interrupt: false,
            bp_regs: vec![0; BP_REG_SIZE],
            cp_regs: vec![0; CP_REG_SIZE],
            xf_mem: vec![0; XF_MEM_SIZE],
            fifo: Vec::with_capacity(256),
        }
    }

    pub fn mmio_write_u8(&mut self, val: u8) {
        self.fifo.push(val);
        self.drain_fifo();
    }

    pub fn mmio_write_u16(&mut self, val: u16) {
        self.fifo.extend_from_slice(&val.to_be_bytes());
        self.drain_fifo();
    }

    pub fn mmio_write_u32(&mut self, val: u32) {
        self.fifo.extend_from_slice(&val.to_be_bytes());
        self.drain_fifo();
    }

    fn drain_fifo(&mut self) {
        let mut pos = 0;

        loop {
            let remaining = self.fifo.len() - pos;
            if remaining == 0 {
                break;
            }

            let cmd = self.fifo[pos];
            match cmd {
                constants::CP_CMD_BYTE => {
                    // 1 cmd + 1 addr + 4 data = 6 bytes
                    if remaining < 6 {
                        break;
                    }
                    let data: [u8; 5] = self.fifo[pos + 1..pos + 6].try_into().unwrap();
                    self.load_cp(&data);
                    pos += 6;
                }
                constants::XF_CMD_BYTE => {
                    // 1 cmd + 2 length + 2 addr = 5 byte header minimum
                    if remaining < 5 {
                        break;
                    }
                    let length = u16::from_be_bytes([self.fifo[pos + 1], self.fifo[pos + 2]]) as usize;
                    let n = length + 1;
                    let total = 5 + n * 4;
                    if remaining < total {
                        break;
                    }
                    let addr = u16::from_be_bytes([self.fifo[pos + 3], self.fifo[pos + 4]]);
                    tracing::debug!(
                        length = length,
                        n = n,
                        addr = format!("{addr:04X}"),
                        total_bytes = total,
                        "XF command parsed"
                    );
                    let data = self.fifo[pos + 1..pos + total].to_vec();
                    self.load_xf(&data);
                    pos += total;
                }
                constants::BP_CMD_BYTE => {
                    // 1 cmd + 4 data = 5 bytes
                    if remaining < 5 {
                        break;
                    }
                    let data: [u8; 4] = self.fifo[pos + 1..pos + 5].try_into().unwrap();
                    self.load_bp(&data);
                    pos += 5;
                }
                _ => {
                    tracing::error!(cmd = format!("{cmd:02X}"), "unknown FIFO command");
                    pos += 1;
                }
            }
        }

        if pos > 0 {
            self.fifo.drain(..pos);
        }
    }

    fn load_bp(&mut self, data: &[u8]) {
        let idx = data[0] as usize;
        let val = u32::from_be_bytes([0, data[1], data[2], data[3]]);
        self.bp_regs[idx] = val;

        tracing::debug!(
            reg_idx = format!("{idx:02X}"),
            value = format!("{val:08X}"),
            "BP register write"
        );

        // PE finish: register 0x45, bit 1
        if idx == 0x45 && (val & 0x02) != 0 {
            self.raise_interrupt = true;
        }
    }

    fn load_cp(&mut self, data: &[u8]) {
        let idx = data[0] as usize;
        let val = u32::from_be_bytes([data[1], data[2], data[3], data[4]]);

        const REG_BASE: usize = 0x20;
        let local = idx.wrapping_sub(REG_BASE);
        if local < self.cp_regs.len() {
            self.cp_regs[local] = val;
        }

        tracing::debug!(
            reg_idx = format!("{idx:02X}"),
            value = format!("{val:08X}"),
            "CP register write"
        );
    }

    fn load_xf(&mut self, data: &[u8]) {
        let length = u16::from_be_bytes([data[0], data[1]]) as usize;
        let addr = u16::from_be_bytes([data[2], data[3]]) as usize;
        let n = length + 1;

        for i in 0..n {
            let offset = 4 + i * 4;
            let val = u32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]);
            let reg = addr + i;
            if reg < self.xf_mem.len() {
                self.xf_mem[reg] = val;
            }

            tracing::debug!(
                reg_idx = format!("{reg:04X}"),
                value = format!("{val:08X}"),
                "XF register write"
            );
        }
    }
}

impl Gekko {
    /// Check if the GX stub detected a finish command and assert the PI interrupt
    pub fn check_gx_pe_finish(&mut self) {
        if self.gx.raise_interrupt {
            self.gx.raise_interrupt = false;
            self.pi.assert_interrupt(InterruptFlag::PeFinish);
        }
    }
}
