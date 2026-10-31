//! Низкоуровневый перехват мыши через Win32 LL-хуки.
//!
//! Предназначен для **стендalone-режима** (см. `demo`): позволяет тестировать
//! логику без запущенной игры. В реальном Horizon-моде события мыши приходят
//! из Java-адаптера (GLFW), поэтому этот модуль поднят за `cfg(windows)` и
//! не участвует в основном пути.
//!
//! ВАЖНО: хук не отправляет клики сам — он лишь транслирует сырые события
//! в колбэк [`install`], откуда их принимает контроллер.

#[cfg(windows)]
use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, MSLLHOOKSTRUCT, WH_MOUSE_LL,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEWHEEL, WM_RBUTTONDOWN, WM_RBUTTONUP,
};

/// Сырое событие мыши для стендалона.
#[derive(Debug, Clone, Copy)]
pub struct RawMouse {
    pub x: f64,
    pub y: f64,
    /// -1 = нет колеса; +1 = вверх; -1? нет: `wheel: i32` (delta/120).
    pub wheel: i32,
    pub left_down: bool,
    pub right_down: bool,
}

type Callback = fn(&RawMouse);

#[cfg(windows)]
mod imp {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

    static CALLBACK: AtomicPtr<()> = AtomicPtr::new(std::ptr::null_mut());
    static RUNNING: AtomicBool = AtomicBool::new(false);
    static mut LAST: RawMouse = RawMouse { x: 0.0, y: 0.0, wheel: 0, left_down: false, right_down: false };

    unsafe extern "system" fn proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        const HC_ACTION: i32 = 0;
        if code == HC_ACTION {
            let info = &*(lparam.0 as *const MSLLHOOKSTRUCT);
            let mut cur = LAST;
            cur.x = info.pt.x as f64;
            cur.y = info.pt.y as f64;
            match wparam.0 as u32 {
                WM_MOUSEWHEEL => {
                    let delta = ((wparam.0 >> 16) & 0xFFFF) as i16;
                    cur.wheel = (delta / 120) as i32;
                }
                WM_LBUTTONDOWN => cur.left_down = true,
                WM_LBUTTONUP => cur.left_down = false,
                WM_RBUTTONDOWN => cur.right_down = true,
                WM_RBUTTONUP => cur.right_down = false,
                _ => {}
            }
            LAST = cur;
            let cb = CALLBACK.load(Ordering::SeqCst);
            if !cb.is_null() {
                // fn-указатель хранился как *mut () — возвращаем тип безопасно
                let f: Callback = unsafe { std::mem::transmute_copy::<*mut (), Callback>(&cb) };
                f(&cur);
            }
        }
        CallNextHookEx(hook_handle(), code, wparam, lparam)
    }

    static mut HOOK: Option<HHOOK> = None;
    unsafe fn hook_handle() -> HHOOK {
        HOOK.unwrap_or(HHOOK::default())
    }

    /// Установить глобальный LL-хук мыши. Возвращает Ok, если хук активен.
    pub unsafe fn install(cb: Callback) -> Result<(), String> {
        if RUNNING.swap(true, Ordering::SeqCst) {
            return Err("хук уже установлен".into());
        }
        CALLBACK.store(cb as *mut (), Ordering::SeqCst);
        let h = SetWindowsHookExW(WH_MOUSE_LL, Some(proc), HINSTANCE::default(), 0)
            .map_err(|e| format!("SetWindowsHookExW: {e}"))?;
        HOOK = Some(h);
        Ok(())
    }

    pub unsafe fn uninstall() {
        if let Some(h) = HOOK.take() {
            let _ = UnhookWindowsHookEx(h);
        }
        RUNNING.store(false, Ordering::SeqCst);
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub unsafe fn install(_cb: Callback) -> Result<(), String> {
        Err("LL-хук мыши доступен только на Windows".into())
    }
    pub unsafe fn uninstall() {}
}

pub use imp::{install, uninstall};
