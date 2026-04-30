use crate::hollywood::ipc::{DeviceContext, IosDevice};

pub struct EventHook;

impl IosDevice for EventHook {
    fn ioctl(
        &mut self,
        _ctx: &mut DeviceContext<'_>,
        cmd: u32,
        in_buf: u32,
        in_len: u32,
        out_buf: u32,
        out_len: u32,
    ) -> i32 {
        tracing::warn!(
            cmd = format!("{cmd:#010X}"),
            in_buf = format!("{in_buf:#010X}"),
            in_len,
            out_buf = format!("{out_buf:#010X}"),
            out_len,
            "STM_EventHook fucked up"
        );
        0
    }
}
