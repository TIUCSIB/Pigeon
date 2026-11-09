//! Windows 窗口外观:为无边框窗口补上 DWM 圆角与画布色边框。
//!
//! `decorations:false` 时 shadow:true 在 Win11 有圆角但带 1px 白色系统边框,
//! 这里显式设置:圆角偏好 ROUND + 边框颜色 = 画布色,观感一致不透色。

#![cfg(windows)]

use windows::Win32::Foundation::{COLORREF, HWND};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE,
    DWM_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
};

/// hwnd 为窗口句柄(isize);dark 决定边框用暗色还是亮色画布色
pub fn apply(hwnd: isize, dark: bool) {
    let pref: DWM_WINDOW_CORNER_PREFERENCE = DWMWCP_ROUND;
    let (r, g, b) = if dark { (0x14, 0x16, 0x1a) } else { (0xf1, 0xf2, 0xf4) };
    let color = COLORREF((r as u32) | ((g as u32) << 8) | ((b as u32) << 16));
    unsafe {
        let hwnd = HWND(hwnd as *mut _);
        // 圆角(Win11 生效;Win10 忽略)
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &pref as *const _ as *const core::ffi::c_void,
            std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        );
        // 边框颜色 = 画布色,盖掉 1px 白框(Win11 22000+)
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            &color as *const _ as *const core::ffi::c_void,
            std::mem::size_of::<COLORREF>() as u32,
        );
    }
}
