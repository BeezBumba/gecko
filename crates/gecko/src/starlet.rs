use crate::system::SystemId;
use crate::{
    System,
    hollywood::ipc::{DeviceContext, IosDevice},
};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

// As per zayd
const FINALIZE_DELAY_CYCLES: u64 = 10_000;
// Give the CPU breathing room between back-to-back IPC IRQs so other interrupt
// sources (DSP) actually get serviced. The BT stub's "no events" replies
// would otherwise hammer the IPC handler nonstop and starve the DSP IRQ.
const ACK_TO_NEXT_DELAY_CYCLES: u64 = 50_000;

pub struct PendingResponse {
    pub cmd_paddr: u32,
    pub result: i32,
}

/// Backing storage for an open fd. Control devices share state across fds via
/// a path key into the registry; host backed files own their device instance
/// directly so each fd has its own cursor.
enum FdEntry {
    Shared(String),
    Owned(Box<dyn IosDevice>),
}

pub struct Starlet {
    devices: HashMap<String, Box<dyn IosDevice>>,
    handles: HashMap<i32, FdEntry>,
    next_fd: i32,
    pub pending: VecDeque<PendingResponse>,
    pub host_fs_root: PathBuf,
}

impl Starlet {
    pub fn new() -> Self {
        Starlet {
            devices: HashMap::new(),
            handles: HashMap::new(),
            next_fd: 1,
            pending: VecDeque::new(),
            host_fs_root: self::default_host_fs_root(),
        }
    }

    /// Install a device at `path`. Replaces any existing registration.
    pub fn register(&mut self, path: &str, dev: Box<dyn IosDevice>) {
        self.devices.insert(path.to_owned(), dev);
    }

    /// Allocate a fresh fd bound to a registered (shared) device path.
    pub fn allocate_fd(&mut self, path: &str) -> i32 {
        let fd = self.fresh_fd();
        self.handles.insert(fd, FdEntry::Shared(path.to_owned()));
        fd
    }

    /// Allocate a fresh fd that owns the given device instance. Used for
    /// per-open resources (host-backed real files).
    pub fn allocate_owned_fd(&mut self, dev: Box<dyn IosDevice>) -> i32 {
        let fd = self.fresh_fd();
        self.handles.insert(fd, FdEntry::Owned(dev));
        fd
    }

    fn fresh_fd(&mut self) -> i32 {
        let fd = self.next_fd;
        self.next_fd = self.next_fd.checked_add(1).expect("Starlet fd overflow");
        fd
    }

    pub fn device_for_path(&mut self, path: &str) -> Option<&mut Box<dyn IosDevice>> {
        self.devices.get_mut(path)
    }

    pub fn device_for_fd(&mut self, fd: i32) -> Option<&mut dyn IosDevice> {
        let path = match self.handles.get(&fd)? {
            FdEntry::Shared(p) => Some(p.clone()),
            FdEntry::Owned(_) => None,
        };

        if let Some(path) = path {
            return self.devices.get_mut(&path).map(|b| b.as_mut() as &mut dyn IosDevice);
        }

        match self.handles.get_mut(&fd)? {
            FdEntry::Owned(dev) => Some(dev.as_mut()),
            FdEntry::Shared(_) => unreachable!(),
        }
    }

    /// Drop an fd. Calls `close` on the underlying device first; for owned
    /// fds the device is dropped after close.
    pub fn close_fd(&mut self, fd: i32, ctx: &mut DeviceContext<'_>) -> i32 {
        let Some(entry) = self.handles.remove(&fd) else {
            return crate::hollywood::ipc::IPC_EINVAL;
        };

        match entry {
            FdEntry::Owned(mut dev) => dev.close(ctx),
            FdEntry::Shared(path) => match self.devices.get_mut(&path) {
                Some(dev) => dev.close(ctx),
                None => crate::hollywood::ipc::IPC_EINVAL,
            },
        }
    }
}

fn default_host_fs_root() -> PathBuf {
    if let Some(custom) = std::env::var_os("GECKO_FS_ROOT") {
        return PathBuf::from(custom);
    }
    PathBuf::from("fs")
}

impl System<{ crate::WII }> {
    pub fn initialize_starlet_devices(&mut self) {
        use crate::hollywood::ipc::{self, stm};

        self.starlet.register("/dev/stm/immediate", Box::new(stm::Immediate));
        self.starlet.register("/dev/stm/eventhook", Box::new(stm::EventHook));
        self.starlet.register("/dev/fs", Box::new(ipc::fs::FileSystem));
        self.starlet
            .register("/dev/di", Box::new(ipc::di::DiskInterface::new()));
        self.starlet.register("/dev/es", Box::new(ipc::es::ETicketServices));
        self.starlet
            .register("/dev/usb/oh1/57e/305", Box::new(ipc::usb::Bluetooth::new()));
    }

    pub fn create_device_context(&mut self) -> (&mut Starlet, DeviceContext<'_>) {
        (
            &mut self.starlet,
            DeviceContext {
                mmio: &mut self.mmio,
                scheduler: &mut self.scheduler,
                di: &mut self.di,
            },
        )
    }
}

pub fn dispatch_command<const SYSTEM: SystemId>(sys: &mut System<SYSTEM>, cmd_paddr: u32) {
    let result = self::process_command(sys, cmd_paddr);
    sys.starlet.pending.push_back(PendingResponse { cmd_paddr, result });
    sys.scheduler
        .schedule_in(FINALIZE_DELAY_CYCLES, self::deliver_pending::<SYSTEM>);
}

pub fn schedule_drain<const SYSTEM: SystemId>(sys: &mut System<SYSTEM>) {
    sys.scheduler
        .schedule_in(ACK_TO_NEXT_DELAY_CYCLES, self::deliver_pending::<SYSTEM>);
}

fn process_command<const SYSTEM: SystemId>(sys: &mut System<SYSTEM>, cmd_paddr: u32) -> i32 {
    use crate::hollywood::ipc::{IPC_EINVAL, IPC_ENOENT};

    const IOS_OPEN: u32 = 1;
    const IOS_CLOSE: u32 = 2;
    const IOS_READ: u32 = 3;
    const IOS_WRITE: u32 = 4;
    const IOS_SEEK: u32 = 5;
    const IOS_IOCTL: u32 = 6;
    const IOS_IOCTLV: u32 = 7;

    assert!(SYSTEM == crate::WII, "Starlet dispatch reached on non-Wii system");

    let wii: &mut crate::Wii = unsafe { ::core::mem::transmute(sys) };
    let cmd = wii.mmio.phys_read_u32(cmd_paddr);
    let fd = wii.mmio.phys_read_u32(cmd_paddr + 0x08) as i32;

    let (starlet, mut ctx) = wii.create_device_context();

    match cmd {
        IOS_OPEN => {
            let path_ptr = ctx.mmio.phys_read_u32(cmd_paddr + 0x0C);
            let mode = ctx.mmio.phys_read_u32(cmd_paddr + 0x10);
            let path = self::read_c_string(&mut ctx, path_ptr);

            let fd = if let Some(dev) = starlet.device_for_path(&path) {
                let rc = dev.open(&mut ctx, mode);
                if rc >= 0 { starlet.allocate_fd(&path) } else { rc }
            } else if let Some(real) =
                crate::hollywood::ipc::fs::HostBackedFile::try_open(&starlet.host_fs_root, &path, mode)
            {
                starlet.allocate_owned_fd(Box::new(real))
            } else {
                tracing::error!(%path, "IOS_Open: no device registered");
                IPC_ENOENT
            };
            tracing::debug!(%path, mode, fd, "IOS_Open");

            fd
        }
        IOS_CLOSE => {
            tracing::debug!(fd, "IOS_Close");
            starlet.close_fd(fd, &mut ctx)
        }
        IOS_READ => {
            let out_ptr = ctx.mmio.phys_read_u32(cmd_paddr + 0x0C);
            let out_len = ctx.mmio.phys_read_u32(cmd_paddr + 0x10);

            tracing::debug!(fd, out_ptr = format!("{out_ptr:#010X}"), out_len, "IOS_Read");

            match starlet.device_for_fd(fd) {
                Some(dev) => dev.read(&mut ctx, out_ptr, out_len),
                None => IPC_EINVAL,
            }
        }
        IOS_WRITE => {
            let in_ptr = ctx.mmio.phys_read_u32(cmd_paddr + 0x0C);
            let in_len = ctx.mmio.phys_read_u32(cmd_paddr + 0x10);

            tracing::debug!(fd, in_ptr = format!("{in_ptr:#010X}"), in_len, "IOS_Write");

            match starlet.device_for_fd(fd) {
                Some(dev) => dev.write(&mut ctx, in_ptr, in_len),
                None => IPC_EINVAL,
            }
        }
        IOS_SEEK => {
            let offset = ctx.mmio.phys_read_u32(cmd_paddr + 0x0C) as i32;
            let whence = ctx.mmio.phys_read_u32(cmd_paddr + 0x10) as i32;

            tracing::debug!(fd, offset, whence, "IOS_Seek");

            match starlet.device_for_fd(fd) {
                Some(dev) => dev.seek(&mut ctx, offset, whence),
                None => IPC_EINVAL,
            }
        }
        IOS_IOCTL => {
            let ioctl_cmd = ctx.mmio.phys_read_u32(cmd_paddr + 0x0C);
            let in_ptr = ctx.mmio.phys_read_u32(cmd_paddr + 0x10);
            let in_len = ctx.mmio.phys_read_u32(cmd_paddr + 0x14);
            let out_ptr = ctx.mmio.phys_read_u32(cmd_paddr + 0x18);
            let out_len = ctx.mmio.phys_read_u32(cmd_paddr + 0x1C);

            tracing::debug!(
                fd,
                ioctl_cmd = format!("{ioctl_cmd:#010X}"),
                in_ptr = format!("{in_ptr:#010X}"),
                in_len,
                out_ptr = format!("{out_ptr:#010X}"),
                out_len,
                "IOS_Ioctl"
            );

            match starlet.device_for_fd(fd) {
                Some(dev) => dev.ioctl(&mut ctx, ioctl_cmd, in_ptr, in_len, out_ptr, out_len),
                None => IPC_EINVAL,
            }
        }
        IOS_IOCTLV => {
            let ioctl_cmd = ctx.mmio.phys_read_u32(cmd_paddr + 0x0C);
            let in_count = ctx.mmio.phys_read_u32(cmd_paddr + 0x10);
            let io_count = ctx.mmio.phys_read_u32(cmd_paddr + 0x14);
            let vec_ptr = ctx.mmio.phys_read_u32(cmd_paddr + 0x18);

            tracing::debug!(fd, ioctl_cmd = format!("{ioctl_cmd:#010X}"), "IOS_Ioctlv");

            match starlet.device_for_fd(fd) {
                Some(dev) => dev.ioctlv(&mut ctx, ioctl_cmd, in_count, io_count, vec_ptr),
                None => IPC_EINVAL,
            }
        }
        other => {
            tracing::error!(cmd = other, "unimplemented IOS command");
            IPC_EINVAL
        }
    }
}

fn read_c_string(ctx: &mut DeviceContext<'_>, paddr: u32) -> String {
    let mut bytes = Vec::with_capacity(64);

    for i in 0..64 {
        let b = ctx.mmio.phys_read_u8(paddr + i);
        if b == 0 {
            break;
        }

        bytes.push(b);
    }

    String::from_utf8_lossy(&bytes).into_owned()
}

fn deliver_pending<const SYSTEM: SystemId>(sys: &mut System<SYSTEM>) {
    if sys.hollywood.ipc.ppcctrl.arm_response() {
        tracing::debug!(
            queue_len = sys.starlet.pending.len(),
            "deliver_pending: arm_response still set, skipping"
        );
        return;
    }

    let Some(p) = sys.starlet.pending.pop_front() else {
        tracing::debug!("deliver_pending: queue empty");
        return;
    };
    tracing::debug!(
        cmd_paddr = format!("{:#010X}", p.cmd_paddr),
        result = p.result,
        remaining = sys.starlet.pending.len(),
        "deliver_pending"
    );
    crate::hollywood::ipc::deliver_response(sys, p.cmd_paddr, p.result);
}
