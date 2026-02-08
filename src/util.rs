#![allow(unused)]

use std::fmt;

pub mod log;


#[derive(Clone, Copy, Debug)]
pub struct DateTime {
    pub year:   u16,
    pub month:  u8,
    pub day:    u8,
    pub hour:   u8,
    pub minute: u8,
    pub second: u8,
}

impl fmt::Display for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.year,
            self.month,
            self.day,
            self.hour,
            self.minute,
            self.second
        )
    }
}


cfg_if::cfg_if!{
    if #[cfg(target_os = "windows")] {
        pub use crate::os::windows::get_local_datetime;
    } else {
        pub use crate::os::default::get_local_datetime;
    }
}