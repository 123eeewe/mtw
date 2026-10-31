//! FFI-слой для хост-адаптера Horizon (Java, JNI).
//!
//! Java-сторона (миксин/инжектор в `HandledScreen.mouseScrolled`,
//! `mouseClicked`, `initTweaks`) собирает снимок слотов в плоские массивы
//! и вызывает функции ниже. Возврат — упакованное i64: код действия + slot + amount.
//!
//! Java объявляет методы как `native` в классе-обёртке и загружает библиотеку:
//! `System.loadLibrary("mousetweaks_rs")`.

use crate::config::ModConfig;
use crate::controller::Controller;
use crate::core::{ClickAction, GuiModel, MouseButton, SlotInfo};
use std::ffi::{c_char, CStr, CString};

use std::sync::{Mutex, MutexGuard};

static CONTROLLER: Mutex<Option<Controller>> = Mutex::new(None);

/// Коды действий (стабильные числа для Java-обёртки).
pub mod code {
    pub const PASS: i32 = 0;
    pub const CANCEL: i32 = 1;
    pub const GIVE_TO_CURSOR: i32 = 2; // slot, amount
    pub const TAKE_FROM_CURSOR: i32 = 3; // slot, amount
    pub const HALF_STACK: i32 = 4; // slot
    pub const WHOLE_STACK: i32 = 5; // slot
    pub const TOSS: i32 = 6; // amount = whole_stack flag
}

fn action_to_tuple(a: &ClickAction) -> (i32, usize, u32) {
    match a {
        ClickAction::Pass => (code::PASS, 0, 0),
        ClickAction::Cancel => (code::CANCEL, 0, 0),
        ClickAction::GiveToCursor { slot, amount } => (code::GIVE_TO_CURSOR, *slot, *amount),
        ClickAction::TakeFromCursor { slot, amount } => (code::TAKE_FROM_CURSOR, *slot, *amount),
        ClickAction::HalfStack { slot } => (code::HALF_STACK, *slot, 0),
        ClickAction::WholeStack { slot } => (code::WHOLE_STACK, *slot, 0),
        ClickAction::Toss { whole_stack, .. } => (code::TOSS, 0, *whole_stack as u32),
        ClickAction::Normal { slot, button } => {
            let b = match button {
                MouseButton::Left => 0,
                MouseButton::Right => 1,
                MouseButton::Middle => 2,
            };
            (7, *slot, b)
        }
    }
}

/// Упаковка результата в i64: [code:16][slot:32][amount:16].
fn pack((code, slot, amount): (i32, usize, u32)) -> i64 {
    ((code as i64 & 0xFFFF) << 48) | (((slot as i64) & 0xFFFF_FFFF) << 16) | (amount as i64 & 0xFFFF)
}

/// Распаковка результата (Java-обёртка использует ту же схему битов).
pub fn unpack(v: i64) -> (i32, usize, u32) {
    (
        ((v >> 48) & 0xFFFF) as i32,
        ((v >> 16) & 0xFFFF_FFFF) as usize,
        (v & 0xFFFF) as u32,
    )
}

/// Инициализация: загрузить конфиг из `<root>/config/mousetweaks_rs.toml`.
///
/// # Safety
/// `root` — валидный NUL-terminated UTF-8 путь либо NULL.
#[no_mangle]
pub unsafe extern "C" fn mt_init(root: *const c_char) {
    let path = if root.is_null() {
        None
    } else {
        let s = unsafe { CStr::from_ptr(root) }.to_string_lossy().into_owned();
        Some(std::path::PathBuf::from(s))
    };
    let cfg = ModConfig::load(path.as_deref());
    *CONTROLLER.lock().unwrap() = Some(Controller::new(cfg));
}

/// Взять лок и гарантированно инициализировать контроллер.
fn lock_controller() -> MutexGuard<'static, Option<Controller>> {
    let mut guard = CONTROLLER.lock().unwrap();
    if guard.is_none() {
        *guard = Some(Controller::new(ModConfig::default()));
    }
    guard
}

/// Скролл над GUI из плоских массивов (главный путь для JNI).
///
/// # Safety
/// Все указатели валидны на время вызова, длины массивов = `slot_count`.
#[allow(clippy::too_many_arguments)]
#[no_mangle]
pub unsafe extern "C" fn mt_scroll(
    mx: f64,
    my: f64,
    raw_up: i32,
    ctrl: i32,
    cursor_item: *const c_char, // NULL = руки пусты
    cursor_count: u32,
    cursor_max: u32,
    slot_count: usize,
    slot_indices: *const u32,
    slot_items: *const *const c_char, // NULL = пустой слот
    slot_counts: *const u32,
    slot_xs: *const f64,
    slot_ys: *const f64,
    slot_wh: *const f64, // [w,h,w,h,...]
) -> i64 {
    let mut model = GuiModel {
        cursor_item: (!cursor_item.is_null())
            .then(|| unsafe { CStr::from_ptr(cursor_item) }.to_string_lossy().into_owned()),
        cursor_count,
        cursor_max_stack: cursor_max.max(1),
        slots: Vec::with_capacity(slot_count),
    };
    for i in 0..slot_count {
        let item = unsafe {
            let p = *slot_items.add(i);
            (!p.is_null()).then(|| CStr::from_ptr(p).to_string_lossy().into_owned())
        };
        model.slots.push(SlotInfo {
            index: unsafe { *slot_indices.add(i) } as usize,
            item,
            count: unsafe { *slot_counts.add(i) },
            max_stack: 64,
            x: unsafe { *slot_xs.add(i) },
            y: unsafe { *slot_ys.add(i) },
            width: unsafe { *slot_wh.add(i * 2) },
            height: unsafe { *slot_wh.add(i * 2 + 1) },
            enabled: true,
        });
    }
    let mut guard = lock_controller();
    let c = guard.as_mut().unwrap();
    let action = c.on_scroll(&model, mx, my, raw_up != 0, ctrl != 0);
    pack(action_to_tuple(&action))
}

/// Вспомогательная модель с одним hover-слотом (press/drag/release).
fn hover_model(mx: f64, my: f64, over_slot: i32) -> GuiModel {
    GuiModel {
        cursor_item: None,
        cursor_count: 0,
        cursor_max_stack: 64,
        slots: if over_slot >= 0 {
            vec![SlotInfo {
                index: over_slot as usize,
                item: Some("_hover".into()),
                count: 1,
                max_stack: 64,
                x: mx - 1.0,
                y: my - 1.0,
                width: 3.0,
                height: 3.0,
                enabled: true,
            }]
        } else {
            vec![]
        },
    }
}

fn map_button(b: i32) -> MouseButton {
    match b {
        0 => MouseButton::Left,
        1 => MouseButton::Right,
        _ => MouseButton::Middle,
    }
}

/// Нажатие кнопки над GUI. `over_slot` = индекс слота под курсором или -1.
/// Возвращает код действия (CANCEL — поглощаем обычный клик и начинаем drag).
#[no_mangle]
pub extern "C" fn mt_press(mx: f64, my: f64, button: i32, ctrl: i32, over_slot: i32) -> i32 {
    let model = hover_model(mx, my, over_slot);
    let mut guard = lock_controller();
    let act = guard.as_mut().unwrap().on_press(&model, mx, my, map_button(button), ctrl != 0);
    action_to_tuple(&act).0
}

/// Движение мыши с зажатой кнопкой. Возвращает packed-действие (PASS если ничего).
#[no_mangle]
pub extern "C" fn mt_drag_move(mx: f64, my: f64, over_slot: i32) -> i64 {
    let model = hover_model(mx, my, over_slot);
    let mut guard = lock_controller();
    let acts = guard.as_mut().unwrap().on_move(&model, mx, my);
    acts
        .first()
        .map(|a| pack(action_to_tuple(a)))
        .unwrap_or(pack((code::PASS, 0, 0)))
}

/// Отпускание кнопки; применяет финальный клик (whole/half stack).
#[no_mangle]
pub extern "C" fn mt_release(mx: f64, my: f64, button: i32, over_slot: i32) -> i64 {
    let model = hover_model(mx, my, over_slot);
    let mut guard = lock_controller();
    let acts = guard.as_mut().unwrap().on_release(&model, mx, my, map_button(button));
    acts
        .first()
        .map(|a| pack(action_to_tuple(a)))
        .unwrap_or(pack((code::PASS, 0, 0)))
}

/// Запрос на выброс стака (Ctrl+Drop / double-click). 1 = cancel, 0 = pass.
#[no_mangle]
pub extern "C" fn mt_toss_attempt(whole_stack: i32) -> i32 {
    let mut guard = lock_controller();
    match guard.as_mut().unwrap().on_toss(whole_stack != 0) {
        ClickAction::Cancel => 1,
        _ => 0,
    }
}

/// Перезагрузка конфига.
///
/// # Safety
/// `root` — валидный NUL-terminated путь либо NULL.
#[no_mangle]
pub unsafe extern "C" fn mt_reload_config(root: *const c_char) {
    unsafe { mt_init(root) }
}

/// HUD-текст (NULL если нет). Освобождать через [`mt_free_str`].
#[no_mangle]
pub extern "C" fn mt_hud_text() -> *mut c_char {
    let mut guard = CONTROLLER.lock().unwrap();
    guard
        .as_mut()
        .and_then(|c| c.hud_text())
        .map(|s| CString::new(s).unwrap_or_default().into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// # Safety
/// Освобождение строки, полученной из [`mt_hud_text`].
#[no_mangle]
pub unsafe extern "C" fn mt_free_str(s: *mut c_char) {
    if !s.is_null() {
        drop(unsafe { CString::from_raw(s) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_roundtrip() {
        let v = pack((code::GIVE_TO_CURSOR, 37, 5));
        assert_eq!(unpack(v), (code::GIVE_TO_CURSOR, 37, 5));
        let v2 = pack((code::PASS, 0, 0));
        assert_eq!(unpack(v2), (code::PASS, 0, 0));
        let v3 = pack((code::TOSS, 0, 1));
        assert_eq!(unpack(v3), (code::TOSS, 0, 1));
    }

    #[test]
    fn toss_cancel() {
        unsafe { mt_init(std::ptr::null()) };
        assert_eq!(mt_toss_attempt(1), 1);
    }

    #[test]
    fn scroll_via_ffi() {
        unsafe { mt_init(std::ptr::null()) };
        let idx = [0u32];
        let item = CString::new("minecraft:diamond").unwrap();
        let items = [item.as_ptr() as *const c_char];
        let counts = [10u32];
        let xs = [0f64];
        let ys = [0f64];
        let wh = [18f64, 18f64];
        let res = unsafe {
            mt_scroll(
                9.0,
                9.0,
                1, // up
                0,
                std::ptr::null(),
                0,
                64,
                1,
                idx.as_ptr(),
                items.as_ptr(),
                counts.as_ptr(),
                xs.as_ptr(),
                ys.as_ptr(),
                wh.as_ptr(),
            )
        };
        let (c, slot, amount) = unpack(res);
        assert_eq!((c, slot, amount), (code::GIVE_TO_CURSOR, 0, 1));
    }

    #[test]
    fn press_release_via_ffi() {
        unsafe { mt_init(std::ptr::null()) };
        assert_eq!(mt_press(9.0, 9.0, 1, 0, 3), code::CANCEL); // ПКМ над слотом — поглощаем
        let mv = mt_drag_move(9.0, 9.0, 3);
        let (mc, mslot, _) = unpack(mv);
        assert_eq!((mc, mslot), (code::HALF_STACK, 3));
        let rel = mt_release(9.0, 9.0, 1, 4);
        let (c, slot, _) = unpack(rel);
        assert_eq!((c, slot), (code::HALF_STACK, 4));
    }
}
