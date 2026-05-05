use crossbeam_channel::Sender;
use gecko::HostInput;
use gecko::flipper::vi::regs::RefreshRate;
use gecko::system::{System, SystemId};
use std::sync::{Arc, Mutex};
use winit::event_loop::EventLoopProxy;

pub struct FrameMessage {
    pub native_hz: f64,
}

pub fn emu_thread<const SYSTEM: SystemId>(
    mut emulator: System<SYSTEM>,
    frame_tx: Sender<FrameMessage>,
    input: Arc<Mutex<HostInput>>,
    proxy: EventLoopProxy<()>,
) {
    loop {
        let input = *input.lock().unwrap();
        emulator.apply_host_input(&input);
        emulator.run_until_vsync();

        let native_hz = match emulator.vi.dcr.video_format().refresh_rate() {
            RefreshRate::Hz60 => 60.0,
            RefreshRate::Hz50 => 50.0,
        };

        if frame_tx.send(FrameMessage { native_hz }).is_err() {
            break;
        }
        let _ = proxy.send_event(());
    }
}
