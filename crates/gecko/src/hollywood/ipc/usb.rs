use crate::hollywood::ipc::IosDevice;

pub struct Usb;

impl IosDevice for Usb {
    fn ioctlv(
        &mut self,
        _ctx: &mut super::DeviceContext<'_>,
        cmd: u32,
        in_count: u32,
        io_count: u32,
        vec_ptr: u32,
    ) -> i32 {
        tracing::warn!(
            cmd = format!("{cmd:#010X}"),
            in_count,
            io_count,
            vec_ptr = format!("{vec_ptr:#010X}"),
            "USB: unimplemented ioctlv"
        );
        0
    }
}
