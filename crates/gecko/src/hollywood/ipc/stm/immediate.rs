use crate::hollywood::ipc::{DeviceContext, IPC_EINVAL, IosDevice};

pub struct Immediate;

impl IosDevice for Immediate {
    fn ioctl(
        &mut self,
        _ctx: &mut DeviceContext<'_>,
        cmd: u32,
        in_buf: u32,
        in_len: u32,
        out_buf: u32,
        out_len: u32,
    ) -> i32 {
        tracing::error!(cmd, in_buf, in_len, out_buf, out_len, "IOS_Ioctl: unimplemented");
        IPC_EINVAL
    }
}
