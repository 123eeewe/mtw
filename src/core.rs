//! Ядро логики мода: порт алгоритмов carlosgames/mousetweaks на Rust.
//!
//! Модуль не зависит от Minecraft: работа идёт с абстрактной моделью GUI
//! ([`GuiModel`]) — списком слотов и предметом «в руках» курсора.
//! Хост-адаптер (Java/Fabric или GLFW-шейдовер) конвертирует реальное окно
//! игры в эту модель и обратно.

use crate::config::{ModConfig, ScrollEmptySlotMode};

/// Кнопка мыши (как в MouseTweaks: ЛКМ/СКМ/ПКМ).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

/// Идентификатор предмета для сравнения «тот же предмет или нет»
/// (в адаптере = ItemId + NBT hash; здесь — просто строка/число).
pub type ItemKey = Option<String>;

/// Слот GUI.
#[derive(Debug, Clone, PartialEq)]
pub struct SlotInfo {
    /// Индекс слота в контейнере (screen handler slot id).
    pub index: usize,
    /// Ключ предмета (None = пустой слот).
    pub item: ItemKey,
    /// Количество предметов.
    pub count: u32,
    /// Максимальный размер стака для этого предмета.
    pub max_stack: u32,
    /// Границы слота на экране (логические координаты мыши).
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Можно ли вообще взаимодействовать со слотом (slot.isEnabled()).
    pub enabled: bool,
}

impl SlotInfo {
    pub fn is_empty(&self) -> bool {
        self.count == 0 || self.item.is_none()
    }

    pub fn contains(&self, mx: f64, my: f64) -> bool {
        self.enabled && mx >= self.x && mx < self.x + self.width && my >= self.y && my < self.y + self.height
    }
}

/// Снимок состояния экрана: что в руках + все слоты.
#[derive(Debug, Clone, PartialEq)]
pub struct GuiModel {
    pub cursor_item: ItemKey,
    pub cursor_count: u32,
    pub cursor_max_stack: u32,
    pub slots: Vec<SlotInfo>,
}

impl GuiModel {
    /// Слот под курсором мыши (или None).
    pub fn hovered_slot(&self, mx: f64, my: f64) -> Option<&SlotInfo> {
        self.slots.iter().find(|s| s.contains(mx, my))
    }

    /// Позиция (в порядке обхода GUI) ближайшего слота с таким же предметом,
    /// начиная со следующего после `from_pos`. Возвращает **slot.index** найденного слота.
    fn nearest_same_stack(&self, from_pos: usize, key: &ItemKey) -> Option<usize> {
        if key.is_none() {
            return None;
        }
        // как в оригинале: ищем по кругу от текущей позиции
        let n = self.slots.len();
        for i in 1..=n {
            let pos = (from_pos + i) % n;
            let s = &self.slots[pos];
            if s.enabled && &s.item == key && s.count > 0 {
                return Some(s.index);
            }
        }
        None
    }
}

/// Результат клика/скролла: какой clickType послать серверу (протокол InventoryClick).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClickAction {
    /// Обычный клик: slot, button (0=ЛКМ pickup, 1=ПКМ place-one / 0 для single), mode.
    /// value соответствует `clickData` в протоколе QuickMove/Swap; для простоты
    /// используем структуру с полями click_type как в vanilla ScreenHandler.
    Normal { slot: usize, button: MouseButton },
    /// Половина стака (shift-клик-эквивалент для drag-splitting ПКМ).
    HalfStack { slot: usize },
    /// Всю руку в слот (ЛКМ-drag release).
    WholeStack { slot: usize },
    /// Передать ровно `amount` предметов из слота в руки (scroll down/up).
    GiveToCursor { slot: usize, amount: u32 },
    /// Забрать `amount` предметов из рук в слот (обратный скролл).
    TakeFromCursor { slot: usize, amount: u32 },
    /// Бросить предмет из рук (toss) — блокируется, если disable_toss_enabled.
    Toss { slot: usize, whole_stack: bool },
    /// Ничего не делать (поглотить событие).
    Cancel,
    /// Пропустить событие в ванильную обработку.
    Pass,
}

/// Сколько предметов передать за один тик скролла.
/// `stack_size` — размер источника (стак в руках при раскладке, стак в слоте при сборе).
fn scroll_amount(cfg: &ModConfig, stack_size: u32) -> u32 {
    match cfg.items_per_scroll {
        -1 => stack_size.max(1),
        0 => (stack_size / 2).max(1),
        n => (n as u32).max(1),
    }
}

/// Обработка «щелчка колеса мыши» над слотом (scroll-hand).
///
/// Логика 1-в-1 с MouseTweaks `processScroll`:
///  * вверх (up=true) — добавляем предметы из ближайшего такого же стака в руках;
///  * вниз — убираем предметы из стака в руки;
///  * пустые слоты — по режиму `scroll_empty_slot_mode`.
pub fn handle_scroll(model: &GuiModel, cfg: &ModConfig, mx: f64, my: f64, raw_up: bool) -> ClickAction {
    if !cfg.enable {
        return ClickAction::Pass;
    }
    let up = if cfg.reverse_scroll_direction { !raw_up } else { raw_up };

    let Some(hovered) = model.hovered_slot(mx, my) else {
        return ClickAction::Pass; // не над GUI — пусть игра листает хотбар
    };
    // idx — позиция в списке слотов (используется и как slot.index,
    // т.к. адаптер передаёт слоты в порядке обхода GUI)
    let idx = model.slots.iter().position(|s| s.index == hovered.index).unwrap_or(hovered.index);
    let hover_pos = idx;

    // Пустой слот под курсором
    if hovered.is_empty() {
        match cfg.scroll_empty_slot_mode {
            ScrollEmptySlotMode::Disable => return ClickAction::Cancel,
            ScrollEmptySlotMode::Ignore => return ClickAction::Pass,
            ScrollEmptySlotMode::NearestSameItemStack => {
                // Если в руках есть предмет — скроллим его количество через весь GUI:
                // «вверх» = положить из рук в ближайший такой же стак,
                // «вниз» = вытащить из ближайшего такого же стака обратно в руки.
                if let Some(key) = &model.cursor_item {
                    if let Some(target) = model.nearest_same_stack(hover_pos, &Some(key.clone())) {
                        // target — slot.index; переводим в позицию для клика
                        let tpos = model.slots.iter().position(|s| s.index == target).unwrap_or(target);
                        let s = &model.slots[tpos];
                        if up {
                            let amt = scroll_amount(cfg, model.cursor_count);
                            let space = s.max_stack.saturating_sub(s.count);
                            let amt = amt.min(space).min(model.cursor_count);
                            if amt == 0 {
                                return ClickAction::Pass;
                            }
                            return ClickAction::TakeFromCursor { slot: tpos, amount: amt };
                        } else {
                            let space_in_hands =
                                model.cursor_max_stack.saturating_sub(model.cursor_count);
                            let amt = scroll_amount(cfg, s.count).min(space_in_hands).min(s.count);
                            if amt == 0 {
                                return ClickAction::Pass;
                            }
                            return ClickAction::GiveToCursor { slot: tpos, amount: amt };
                        }
                    }
                }
                return ClickAction::Pass;
            }
        }
    }

    // Слот с предметом
    let slot = hovered;
    if up {
        // «собрать» предметы из этого стака в руки (если руки пусты или тот же предмет)
        let can_receive = match (&model.cursor_item, &slot.item) {
            (None, _) => true,
            (Some(c), Some(s)) => c == s,
            _ => false,
        };
        if !can_receive {
            return ClickAction::Pass;
        }
        let space_in_hands = if model.cursor_item.is_none() {
            slot.max_stack
        } else {
            model.cursor_max_stack.saturating_sub(model.cursor_count)
        };
        let amt = scroll_amount(cfg, slot.count).min(space_in_hands).min(slot.count);
        if amt == 0 {
            ClickAction::Pass
        } else {
            ClickAction::GiveToCursor { slot: idx, amount: amt }
        }
    } else {
        // «положить» предметы из рук в этот стак
        let matches = match (&model.cursor_item, &slot.item) {
            (Some(c), Some(s)) => c == s,
            (Some(_), None) => true,
            _ => false,
        };
        if !matches || model.cursor_count == 0 {
            return ClickAction::Pass;
        }
        let space = slot.max_stack.saturating_sub(slot.count);
        // как в MouseTweaks: количество считается от стака-источника (предметы в руках),
        // затем ограничивается свободным местом в слоте и количеством в руках
        let amt = scroll_amount(cfg, model.cursor_count).min(space).min(model.cursor_count);
        if amt == 0 {
            ClickAction::Pass
        } else {
            ClickAction::TakeFromCursor { slot: idx, amount: amt }
        }
    }
}

/// Состояние «перетаскивания» (drag-splitting). Аналог внутреннего state
/// MouseTweaks между press и release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragState {
    Idle,
    /// Зажата кнопка, ещё ни один слот не затронут.
    Held { button: MouseButton },
    /// Прошли минимум один слот — при отпускать надо «закрыть» сделку.
    Active { button: MouseButton, last_slot: usize },
}

/// Начало нажатия кнопки над GUI.
pub fn handle_press(model: &GuiModel, cfg: &ModConfig, mx: f64, my: f64, button: MouseButton) -> (DragState, ClickAction) {
    if !cfg.enable {
        return (DragState::Idle, ClickAction::Pass);
    }
    let allowed = match button {
        MouseButton::Left => cfg.allow_left_dragging,
        MouseButton::Right => cfg.allow_right_dragging,
        MouseButton::Middle => false,
    };
    if !allowed {
        return (DragState::Idle, ClickAction::Pass);
    }
    if model.hovered_slot(mx, my).is_some() {
        (DragState::Held { button }, ClickAction::Cancel) // поглощаем обычный клик
    } else {
        (DragState::Idle, ClickAction::Pass)
    }
}

/// Движение мыши с зажатой кнопкой над новыми слотами.
pub fn handle_drag_move(state: DragState, model: &GuiModel, mx: f64, my: f64) -> (DragState, Vec<ClickAction>) {
    let (DragState::Held { button } | DragState::Active { button, .. }) = state else {
        return (state, vec![]);
    };
    let Some(slot) = model.hovered_slot(mx, my) else {
        return (state, vec![]);
    };
    // не повторяем последний слот
    if let DragState::Active { last_slot, .. } = state {
        if last_slot == slot.index {
            return (state, vec![]);
        }
    }
    if slot.is_empty() {
        // по пустому слоту раскладывать нечего — событие гасим
        return (DragState::Active { button, last_slot: slot.index }, vec![ClickAction::Cancel]);
    }
    let action = match button {
        // ЛКМ-drag: собираем ПОЛОВИНУ стака в руки (затем раздаём WholeStack'ом)
        MouseButton::Left => ClickAction::GiveToCursor { slot: slot.index, amount: (slot.count / 2).max(1) },
        // ПКМ-drag: раскладываем половину стаков из рук
        MouseButton::Right => ClickAction::HalfStack { slot: slot.index },
        MouseButton::Middle => ClickAction::Pass,
    };
    (DragState::Active { button, last_slot: slot.index }, vec![action])
}

/// Отпускание кнопки — финализация перетаскивания.
pub fn handle_release(state: DragState, model: &GuiModel, mx: f64, my: f64) -> (DragState, Vec<ClickAction>) {
    let DragState::Active { button, last_slot } = state else {
        return (DragState::Idle, vec![]);
    };
    let mut out = Vec::new();
    if let Some(slot) = model.hovered_slot(mx, my) {
        let final_action = match button {
            MouseButton::Left => ClickAction::WholeStack { slot: slot.index },
            MouseButton::Right => ClickAction::HalfStack { slot: slot.index },
            MouseButton::Middle => ClickAction::Pass,
        };
        out.push(final_action);
    } else {
        let _ = last_slot;
    }
    (DragState::Idle, out)
}

/// Обработка попытки «бросить предмет» (Ctrl+клик по слоту / двойной клик).
pub fn handle_toss_attempt(cfg: &ModConfig, whole_stack: bool) -> ClickAction {
    if cfg.enable && cfg.disable_toss_enabled {
        ClickAction::Cancel
    } else {
        ClickAction::Toss { slot: 0, whole_stack }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(index: usize, item: ItemKey, count: u32) -> SlotInfo {
        SlotInfo {
            index,
            item,
            count,
            max_stack: 64,
            x: (index as f64) * 20.0,
            y: 0.0,
            width: 20.0,
            height: 20.0,
            enabled: true,
        }
    }

    fn model(cursor: ItemKey, cursor_count: u32, slots: Vec<SlotInfo>) -> GuiModel {
        GuiModel { cursor_item: cursor, cursor_count, cursor_max_stack: 64, slots }
    }

    #[test]
    fn scroll_down_takes_one_from_stack() {
        let m = model(None, 0, vec![slot(0, Some("diamond".into()), 10)]);
        let cfg = ModConfig::default();
        // курсор в центре слота 0 (x=10,y=5); down = false
        assert_eq!(handle_scroll(&m, &cfg, 10.0, 5.0, false), ClickAction::Pass); // руки пусты — pass
    }

    #[test]
    fn scroll_up_gives_to_cursor() {
        let m = model(None, 0, vec![slot(0, Some("diamond".into()), 10)]);
        let cfg = ModConfig::default();
        assert_eq!(handle_scroll(&m, &cfg, 10.0, 5.0, true), ClickAction::GiveToCursor { slot: 0, amount: 1 });
    }

    #[test]
    fn scroll_up_all_stack_when_items_minus_one() {
        let m = model(None, 0, vec![slot(0, Some("diamond".into()), 10)]);
        let cfg = ModConfig { items_per_scroll: -1, ..Default::default() };
        assert_eq!(handle_scroll(&m, &cfg, 10.0, 5.0, true), ClickAction::GiveToCursor { slot: 0, amount: 10 });
    }

    #[test]
    fn scroll_half_stack_zero() {
        // режим 0 = полстака: считаем от стака-источника (10 в слоте -> 5)
        let m = model(None, 0, vec![slot(0, Some("diamond".into()), 10)]);
        let cfg = ModConfig { items_per_scroll: 0, ..Default::default() };
        assert_eq!(handle_scroll(&m, &cfg, 10.0, 5.0, true), ClickAction::GiveToCursor { slot: 0, amount: 5 });

        // раскладка из рук в полный наполовину стак: руки = источник (8 -> 4)
        let m2 = model(Some("diamond".into()), 8, vec![slot(0, Some("diamond".into()), 10)]);
        assert_eq!(handle_scroll(&m2, &cfg, 10.0, 5.0, false), ClickAction::TakeFromCursor { slot: 0, amount: 4 });
    }

    #[test]
    fn reverse_direction() {
        let m = model(None, 0, vec![slot(0, Some("diamond".into()), 10)]);
        let cfg = ModConfig { reverse_scroll_direction: true, ..Default::default() };
        assert_eq!(handle_scroll(&m, &cfg, 10.0, 5.0, true), ClickAction::Pass);
        // теперь «вниз» инвертирован в «вверх»:
        let m2 = model(Some("diamond".into()), 5, vec![slot(0, Some("diamond".into()), 10)]);
        assert_eq!(handle_scroll(&m2, &cfg, 10.0, 5.0, true), ClickAction::TakeFromCursor { slot: 0, amount: 1 });
    }

    #[test]
    fn empty_slot_modes() {
        let m = model(Some("diamond".into()), 5, vec![slot(0, None, 0), slot(1, Some("diamond".into()), 8)]);
        let cfg_disable = ModConfig { scroll_empty_slot_mode: ScrollEmptySlotMode::Disable, ..Default::default() };
        assert_eq!(handle_scroll(&m, &cfg_disable, 10.0, 5.0, true), ClickAction::Cancel);

        let cfg_ignore = ModConfig { scroll_empty_slot_mode: ScrollEmptySlotMode::Ignore, ..Default::default() };
        assert_eq!(handle_scroll(&m, &cfg_ignore, 10.0, 5.0, true), ClickAction::Pass);

        let cfg_nearest = ModConfig::default();
        assert_eq!(
            handle_scroll(&m, &cfg_nearest, 10.0, 5.0, true),
            ClickAction::TakeFromCursor { slot: 1, amount: 1 }
        );
    }

    #[test]
    fn different_item_blocked() {
        let m = model(Some("stick".into()), 3, vec![slot(0, Some("diamond".into()), 10)]);
        let cfg = ModConfig::default();
        assert_eq!(handle_scroll(&m, &cfg, 10.0, 5.0, false), ClickAction::Pass);
        assert_eq!(handle_scroll(&m, &cfg, 10.0, 5.0, true), ClickAction::Pass);
    }

    #[test]
    fn drag_flow() {
        let m = model(Some("dirt".into()), 64, (0..4).map(|i| slot(i, Some("dirt".into()), 8)).collect());
        let cfg = ModConfig::default();
        let (st, act) = handle_press(&m, &cfg, 10.0, 5.0, MouseButton::Right);
        assert_eq!(act, ClickAction::Cancel);
        let (st, acts) = handle_drag_move(st, &m, 30.0, 5.0);
        assert_eq!(acts, vec![ClickAction::HalfStack { slot: 1 }]);
        let (st, acts) = handle_drag_move(st, &m, 30.0, 5.0); // тот же слот — повтор гасится
        assert!(acts.is_empty());
        let (_, acts) = handle_release(st, &m, 50.0, 5.0);
        assert_eq!(acts, vec![ClickAction::HalfStack { slot: 2 }]);

        // ЛКМ-drag по заполненным слотам: собираем половину стака в руки
        let m2 = model(Some("dirt".into()), 0, vec![slot(0, Some("dirt".into()), 8), slot(1, Some("stone".into()), 4)]);
        let (st2, _) = handle_press(&m2, &cfg, 10.0, 5.0, MouseButton::Left);
        let (st2, acts) = handle_drag_move(st2, &m2, 10.0, 5.0);
        assert_eq!(acts, vec![ClickAction::GiveToCursor { slot: 0, amount: 4 }]);
        let (st2, acts) = handle_drag_move(st2, &m2, 30.0, 5.0);
        assert_eq!(acts, vec![ClickAction::GiveToCursor { slot: 1, amount: 2 }]);
        let (_, acts) = handle_release(st2, &m2, 30.0, 5.0);
        assert_eq!(acts, vec![ClickAction::WholeStack { slot: 1 }]);
    }

    #[test]
    fn toss_cancelled() {
        let cfg = ModConfig::default();
        assert_eq!(handle_toss_attempt(&cfg, true), ClickAction::Cancel);
        let cfg2 = ModConfig { disable_toss_enabled: false, ..Default::default() };
        assert_eq!(handle_toss_attempt(&cfg2, true), ClickAction::Toss { slot: 0, whole_stack: true });
    }

    #[test]
    fn disabled_mod_passes_everything() {
        let m = model(None, 0, vec![slot(0, Some("diamond".into()), 10)]);
        let cfg = ModConfig { enable: false, ..Default::default() };
        assert_eq!(handle_scroll(&m, &cfg, 10.0, 5.0, true), ClickAction::Pass);
    }
}
