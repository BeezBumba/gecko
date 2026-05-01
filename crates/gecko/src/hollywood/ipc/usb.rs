use crate::hollywood::ipc::{DeviceContext, IosDevice};
use std::collections::VecDeque;

const USB_CTRL: u32 = 0;
const USB_BULK: u32 = 1;
const USB_INTR: u32 = 2;
const EP_INT_IN: u8 = 0x81;

const EV_COMMAND_COMPLETE: u8 = 0x0E;
const OP_READ_LOCAL_VERSION: u16 = 0x1001;
const OP_READ_LOCAL_FEATURES: u16 = 0x1003;
const OP_READ_BUFFER_SIZE: u16 = 0x1005;
const OP_READ_BD_ADDR: u16 = 0x1009;

pub struct Bluetooth {
    pending_hci: VecDeque<Vec<u8>>,
}

impl Bluetooth {
    pub fn new() -> Self {
        Self {
            pending_hci: VecDeque::new(),
        }
    }
}

impl IosDevice for Bluetooth {
    fn ioctlv(&mut self, ctx: &mut DeviceContext<'_>, cmd: u32, _in_count: u32, _io_count: u32, vec_ptr: u32) -> i32 {
        match cmd {
            USB_CTRL => self.handle_control(ctx, vec_ptr),
            USB_INTR => self.handle_interrupt(ctx, vec_ptr),
            USB_BULK => self.handle_bulk(ctx, vec_ptr),
            _ => 0,
        }
    }
}

impl Bluetooth {
    fn handle_control(&mut self, ctx: &mut DeviceContext<'_>, vec_ptr: u32) -> i32 {
        let bm_request = ctx.mmio.phys_read_u8(self::vec_data(ctx.mmio, vec_ptr, 0));
        let b_request = ctx.mmio.phys_read_u8(self::vec_data(ctx.mmio, vec_ptr, 1));
        if bm_request != 0x20 || b_request != 0 {
            return 0;
        }

        let w_length = {
            let p = self::vec_data(ctx.mmio, vec_ptr, 4);
            u16::from_le_bytes([ctx.mmio.phys_read_u8(p), ctx.mmio.phys_read_u8(p + 1)])
        };
        if w_length < 3 {
            return 0;
        }

        let data_ptr = self::vec_data(ctx.mmio, vec_ptr, 6);
        let opcode = u16::from_le_bytes([ctx.mmio.phys_read_u8(data_ptr), ctx.mmio.phys_read_u8(data_ptr + 1)]);

        self.queue_command_complete(opcode);
        0
    }

    /// For bulk OUT (0x02) we currently drop whatever the SDK sends. We warn if
    /// there's any payload, since Bulk IN (0x82) just gets a length 0 reply
    /// since we don't have ACL frames queued.
    fn handle_bulk(&mut self, ctx: &mut DeviceContext<'_>, vec_ptr: u32) -> i32 {
        let endpoint = ctx.mmio.phys_read_u8(self::vec_data(ctx.mmio, vec_ptr, 0));

        if endpoint == 0x02 {
            let len_p = self::vec_data(ctx.mmio, vec_ptr, 1);
            let w_length = u16::from_le_bytes([ctx.mmio.phys_read_u8(len_p), ctx.mmio.phys_read_u8(len_p + 1)]);
            if w_length > 0 {
                tracing::warn!(len = w_length, "no L2CAP yet");
            }
        }

        0
    }

    /// HCI event receive (interrupt IN endpoint 0x81). Deliver the next
    /// queued event, or reply length 0 (the SDK seems to tolerate that).
    fn handle_interrupt(&mut self, ctx: &mut DeviceContext<'_>, vec_ptr: u32) -> i32 {
        if ctx.mmio.phys_read_u8(self::vec_data(ctx.mmio, vec_ptr, 0)) != EP_INT_IN {
            return 0;
        }

        let Some(event) = self.pending_hci.pop_front() else {
            return 0;
        };

        let buf_ptr = self::vec_data(ctx.mmio, vec_ptr, 2);
        for (i, b) in event.iter().enumerate() {
            ctx.mmio.phys_write_u8(buf_ptr + i as u32, *b);
        }

        event.len() as i32
    }

    /// Build an HCI Command_Complete event for `opcode` and append it to the
    /// outgoing queue. The four init Read_* commands need real payloads so
    /// the SDK's BTM init state machine can advance to state 5; everything
    /// else is generic status=0.
    fn queue_command_complete(&mut self, opcode: u16) {
        let payload: &[u8] = match opcode {
            OP_READ_LOCAL_VERSION => &[
                0x00, // status
                0x03, // HCI v1.2
                0x40, 0x0E, // HCI revision
                0x03, // LMP v1.2
                0x0F, 0x00, // manufacturer = Broadcom
                0x40, 0x0E, // LMP subversion
            ],
            OP_READ_LOCAL_FEATURES => &[0x00, 0xBC, 0x02, 0x04, 0x38, 0x08, 0x08, 0x00, 0x00],
            OP_READ_BUFFER_SIZE => &[
                0x00, 0x53, 0x01, // ACL packet length = 339
                0x40, // SCO packet length = 64
                0x08, 0x00, // num ACL packets
                0x08, 0x00, // num SCO packets
            ],
            OP_READ_BD_ADDR => &[0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66],
            _ => &[0x00],
        };

        let mut event = Vec::with_capacity(5 + payload.len());
        event.extend_from_slice(&[
            EV_COMMAND_COMPLETE,
            (3 + payload.len()) as u8,
            0x01, // num_hci_command_packets
            opcode as u8,
            (opcode >> 8) as u8,
        ]);
        event.extend_from_slice(payload);

        self.pending_hci.push_back(event);
    }
}

fn vec_data<const SYS: crate::system::SystemId>(mmio: &crate::mmio::Mmio<SYS>, vec_ptr: u32, idx: u32) -> u32 {
    mmio.phys_read_u32(vec_ptr + (idx * 8))
}
