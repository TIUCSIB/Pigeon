//! Windows 窗口外观:无边框窗口的 DWM 圆角、画布色边框。
//!
//! `decorations:false` + `shadow:true` 时,tao 为了给 DWM 阴影留位置,会在窗口里
//! 留一圈原生非客户区(左右/下 = SM_CXSIZEFRAME + SM_CXPADDEDBORDER,顶部约
//! 1~2px,见 tao 的 calculate_insets_for_dpi),WebView 只铺客户区。这圈区域由
//! 系统按"浅色窗口边框"绘制成白色,不受应用主题影响,所以暗色下会看到一圈白边。
//! DWMWA_BORDER_COLOR 只改最外 1px,DWMWA_CAPTION_COLOR 只改顶部标题区,都盖不住它。
//! 这里用 WM_NCPAINT 子类化把这圈非客户区直接涂成画布色。

#![cfg(windows)]

use std::sync::atomic::{AtomicU32, Ordering};

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE,
    DWM_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
};
use windows::Win32::Graphics::Gdi::{
    ClientToScreen, CreateSolidBrush, DeleteObject, FillRect, GetWindowDC, RedrawWindow, ReleaseDC,
    HGDIOBJ, RDW_FRAME, RDW_INVALIDATE, RDW_UPDATENOW,
};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClientRect, GetWindowRect, WM_NCACTIVATE, WM_NCPAINT, WM_SHOWWINDOW,
};

/// 非客户区填充色(COLORREF 0x00BBGGRR),跟随画布色
static FRAME_COLOR: AtomicU32 = AtomicU32::new(0x00F4F2F1);

/// 画布色:亮 #f1f2f4 / 暗 #14161a
fn canvas_color(dark: bool) -> u32 {
    if dark {
        0x001A1614
    } else {
        0x00F4F2F1
    }
}

/// hwnd 为窗口句柄(isize);dark 决定边框用暗色还是亮色画布色
pub fn apply(hwnd: isize, dark: bool) {
    let hwnd = HWND(hwnd as *mut _);
    let color = canvas_color(dark);
    FRAME_COLOR.store(color, Ordering::Relaxed);
    let cref = COLORREF(color);
    let ptr = &cref as *const _ as *const core::ffi::c_void;

    unsafe {
        // 圆角(Win11 生效;Win10 忽略)
        let pref: DWM_WINDOW_CORNER_PREFERENCE = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &pref as *const _ as *const core::ffi::c_void,
            std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        );
        // 最外 1px 的 DWM 边框、顶部标题区都对齐画布色
        for attr in [DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR] {
            let _ = DwmSetWindowAttribute(
                hwnd,
                attr,
                ptr,
                std::mem::size_of::<COLORREF>() as u32,
            );
        }
        // 阴影预留的非客户区:挂一次子类过程,由 WM_NCPAINT 涂画布色
        let _ = SetWindowSubclass(hwnd, Some(nc_paint), 1, 0);
        // 立即重画边框(主题切换时需要)
        let _ = RedrawWindow(
            Some(hwnd),
            None,
            None,
            RDW_FRAME | RDW_INVALIDATE | RDW_UPDATENOW,
        );
    }
}

/// WM_NCPAINT:窗口边框重绘时,用画布色铺满客户区之外的四条边。
/// 另外在显示/焦点切换后补一次整框重绘——窗口首次显示时系统只失效部分边框,
/// 不整框重画会残留系统默认边框的像素(白线/强调色细线、标题图标残片)。
unsafe extern "system" fn nc_paint(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    if msg == WM_SHOWWINDOW || msg == WM_NCACTIVATE {
        let result = DefSubclassProc(hwnd, msg, wparam, lparam);
        let _ = RedrawWindow(
            Some(hwnd),
            None,
            None,
            RDW_FRAME | RDW_INVALIDATE | RDW_UPDATENOW,
        );
        return result;
    }
    if msg != WM_NCPAINT {
        return DefSubclassProc(hwnd, msg, wparam, lparam);
    }

    let mut win = RECT::default();
    let mut client = RECT::default();
    let mut origin = POINT { x: 0, y: 0 };
    if GetWindowRect(hwnd, &mut win).is_err()
        || GetClientRect(hwnd, &mut client).is_err()
        || !ClientToScreen(hwnd, &mut origin).as_bool()
    {
        return DefSubclassProc(hwnd, msg, wparam, lparam);
    }

    // 客户区在窗口坐标系里的左上角与尺寸
    let cx = origin.x - win.left;
    let cy = origin.y - win.top;
    let (w, h) = (win.right - win.left, win.bottom - win.top);
    let (cw, ch) = (client.right, client.bottom);
    if cw <= 0 || ch <= 0 {
        return LRESULT(0);
    }

    let dc = GetWindowDC(Some(hwnd));
    if dc.is_invalid() {
        return LRESULT(0);
    }
    let brush = CreateSolidBrush(COLORREF(FRAME_COLOR.load(Ordering::Relaxed)));
    // 上、下、左、右四条边;只铺客户区之外,不碰 WebView
    let strips = [
        RECT { left: 0, top: 0, right: w, bottom: cy },
        RECT { left: 0, top: cy + ch, right: w, bottom: h },
        RECT { left: 0, top: cy, right: cx, bottom: cy + ch },
        RECT { left: cx + cw, top: cy, right: w, bottom: cy + ch },
    ];
    for r in &strips {
        if r.right > r.left && r.bottom > r.top {
            FillRect(dc, r, brush);
        }
    }
    let _ = DeleteObject(HGDIOBJ(brush.0));
    ReleaseDC(Some(hwnd), dc);
    LRESULT(0)
}
