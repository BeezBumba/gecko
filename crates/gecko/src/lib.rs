pub mod audio;
pub mod common;
pub mod dvd;
pub mod flipper;
pub mod gamecube;
pub mod gekko;
pub mod hollywood;
pub mod host;
pub mod idle;
pub mod ipl;
pub mod mmio;
pub mod scheduler;
pub mod starlet;
pub mod system;
pub mod wii;
pub mod input;

pub use gamecube::GameCube;
pub use system::{GC, System, SystemId, WII};
pub use wii::Wii;
pub use input::HostInput;

#[cfg(feature = "hooks")]
pub mod hooks;
