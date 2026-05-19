use gecko::flipper::si::pad::{self, PadStatus, STICK_CENTER, STICK_MAX, STICK_MIN, TRIGGER_MAX, TRIGGER_MIN};
use gecko::hollywood::ipc::usb as wiimote;
use iced::keyboard::key::Code;

#[inline(always)]
fn set_bit<T: std::ops::BitOrAssign + std::ops::BitAndAssign + std::ops::Not<Output = T> + Copy>(
    bits: &mut T,
    mask: T,
    on: bool,
) {
    if on {
        *bits |= mask;
    } else {
        *bits &= !mask;
    }
}

pub fn update_pad(pad: &mut PadStatus, key: Code, pressed: bool) {
    match key {
        Code::ArrowUp => pad.stick_y = if pressed { STICK_MAX } else { STICK_CENTER },
        Code::ArrowDown => pad.stick_y = if pressed { STICK_MIN } else { STICK_CENTER },
        Code::ArrowLeft => pad.stick_x = if pressed { STICK_MIN } else { STICK_CENTER },
        Code::ArrowRight => pad.stick_x = if pressed { STICK_MAX } else { STICK_CENTER },

        Code::KeyX => self::set_bit(&mut pad.buttons, pad::A, pressed),
        Code::KeyZ => self::set_bit(&mut pad.buttons, pad::B, pressed),
        Code::KeyC => self::set_bit(&mut pad.buttons, pad::X, pressed),
        Code::KeyV => self::set_bit(&mut pad.buttons, pad::Y, pressed),
        Code::Enter => self::set_bit(&mut pad.buttons, pad::START, pressed),

        Code::KeyA => {
            self::set_bit(&mut pad.buttons, pad::L, pressed);
            pad.trigger_left = if pressed { TRIGGER_MAX } else { TRIGGER_MIN };
        }
        Code::KeyS => {
            self::set_bit(&mut pad.buttons, pad::R, pressed);
            pad.trigger_right = if pressed { TRIGGER_MAX } else { TRIGGER_MIN };
        }
        Code::KeyD => self::set_bit(&mut pad.buttons, pad::Z, pressed),

        Code::KeyI => self::set_bit(&mut pad.buttons, pad::DPAD_UP, pressed),
        Code::KeyK => self::set_bit(&mut pad.buttons, pad::DPAD_DOWN, pressed),
        Code::KeyJ => self::set_bit(&mut pad.buttons, pad::DPAD_LEFT, pressed),
        Code::KeyL => self::set_bit(&mut pad.buttons, pad::DPAD_RIGHT, pressed),

        _ => {}
    }
}

pub fn update_wiimote_keys(buttons: &mut u16, key: Code, pressed: bool) {
    let mask = match key {
        Code::Digit1 => wiimote::BTN_ONE,
        Code::Digit2 => wiimote::BTN_TWO,
        Code::Home => wiimote::BTN_HOME,
        Code::Minus => wiimote::BTN_MINUS,
        Code::Equal => wiimote::BTN_PLUS,
        Code::ArrowUp => wiimote::BTN_UP,
        Code::ArrowDown => wiimote::BTN_DOWN,
        Code::ArrowLeft => wiimote::BTN_LEFT,
        Code::ArrowRight => wiimote::BTN_RIGHT,
        _ => return,
    };
    self::set_bit(buttons, mask, pressed);
}

pub fn update_wiimote_motion_keys(shake: &mut bool, key: Code, pressed: bool) {
    if matches!(key, Code::ShiftLeft) {
        *shake = pressed;
    }
}

pub fn update_nunchuk_keys(buttons: &mut u8, stick_x: &mut u8, stick_y: &mut u8, key: Code, pressed: bool) {
    use wiimote::{NUNCHUK_STICK_CENTER as C, NUNCHUK_STICK_MAX as MAX, NUNCHUK_STICK_MIN as MIN};
    match key {
        Code::KeyW => *stick_y = if pressed { MAX } else { C },
        Code::KeyS => *stick_y = if pressed { MIN } else { C },
        Code::KeyA => *stick_x = if pressed { MIN } else { C },
        Code::KeyD => *stick_x = if pressed { MAX } else { C },
        Code::KeyQ => self::set_bit(buttons, wiimote::NUNCHUK_BTN_Z, pressed),
        Code::KeyE => self::set_bit(buttons, wiimote::NUNCHUK_BTN_C, pressed),
        _ => {}
    }
}
