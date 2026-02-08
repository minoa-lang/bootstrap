use crate::util::DateTime;

pub use windows;


pub fn get_local_datetime() -> DateTime {
    let sys_time = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };

    DateTime {
        year: sys_time.wYear,
        month: sys_time.wMonth as u8,
        day: sys_time.wDay as u8,
        hour: sys_time.wHour as u8,
        minute: sys_time.wMinute as u8,
        second: sys_time.wSecond as u8,
    }
}