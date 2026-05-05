use crate::{SystemId, flipper::si::pad, WII, GC};

#[derive(Clone, Copy, Debug)]
pub enum HostInput {
    Gc(pad::PadStatus),
    Wii {
        wiimote_buttons: u16,
        nunchuk_buttons: u8,
        nunchuk_stick_x: u8,
        nunchuk_stick_y: u8,
    },
}

impl HostInput {
    pub fn gc_connected() -> Self {
        Self::Gc(pad::PadStatus {
            connected: true,
            ..pad::PadStatus::default()
        })
    }

    pub fn wii_neutral() -> Self {
        Self::Wii {
            wiimote_buttons: 0,
            nunchuk_buttons: 0,
            nunchuk_stick_x: 0x80,
            nunchuk_stick_y: 0x80,
        }
    }

    pub fn neutral_for(system: SystemId) -> Self {
        match system {
            WII => Self::wii_neutral(),
            GC => Self::gc_connected(),
            _ => unreachable!(),
        }
    }
}