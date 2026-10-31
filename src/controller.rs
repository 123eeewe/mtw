//! Оркестратор: связывает конфиг, состояние drag и чистые функции ядра.
//! Экземпляр живёт один на клиент (в статике за mutex'ом для FFI).

use crate::config::{ModConfig, ScrollEmptySlotMode};
use crate::core::{handle_drag_move, handle_press, handle_release, handle_scroll, handle_toss_attempt, ClickAction, DragState, GuiModel, MouseButton};

pub struct Controller {
    pub cfg: ModConfig,
    pub drag: DragState,
}

impl Default for Controller {
    fn default() -> Self {
        Self { cfg: ModConfig::default(), drag: DragState::Idle }
    }
}

impl Controller {
    pub fn new(cfg: ModConfig) -> Self {
        Self { cfg, drag: DragState::Idle }
    }

    /// Скролл над GUI. `ctrl_held` — учёт настройки disable_with_ctrl.
    pub fn on_scroll(&mut self, model: &GuiModel, mx: f64, my: f64, raw_up: bool, ctrl_held: bool) -> ClickAction {
        if self.cfg.disable_with_ctrl && ctrl_held {
            return ClickAction::Pass;
        }
        handle_scroll(model, &self.cfg, mx, my, raw_up)
    }

    pub fn on_press(&mut self, model: &GuiModel, mx: f64, my: f64, button: MouseButton, ctrl_held: bool) -> ClickAction {
        if self.cfg.disable_with_ctrl && ctrl_held {
            self.drag = DragState::Idle;
            return ClickAction::Pass;
        }
        let (st, act) = handle_press(model, &self.cfg, mx, my, button);
        self.drag = st;
        act
    }

    pub fn on_move(&mut self, model: &GuiModel, mx: f64, my: f64) -> Vec<ClickAction> {
        let (st, acts) = handle_drag_move(self.drag, model, mx, my);
        self.drag = st;
        acts
    }

    pub fn on_release(&mut self, model: &GuiModel, mx: f64, my: f64, button: MouseButton) -> Vec<ClickAction> {
        // release только «своей» кнопки
        let matches = match self.drag {
            DragState::Held { button: b } | DragState::Active { button: b, .. } => b == button,
            DragState::Idle => false,
        };
        if !matches {
            return vec![];
        }
        let (st, acts) = handle_release(self.drag, model, mx, my);
        self.drag = st;
        acts
    }

    pub fn on_toss(&mut self, whole_stack: bool) -> ClickAction {
        handle_toss_attempt(&self.cfg, whole_stack)
    }

    /// Строка для HUD-оверлея (debug), аналог подсказок оригинала.
    pub fn hud_text(&self) -> Option<String> {
        if !self.cfg.show_hud_overlay {
            return None;
        }
        match self.drag {
            DragState::Idle => None,
            DragState::Held { button } => Some(format!("MT-RS: держу {:?}, веду по слотам…", button)),
            DragState::Active { button, last_slot } => Some(format!(
                "MT-RS: {:?}-drag, пустой-режим={}, последний слот #{last_slot}",
                button,
                self.cfg.scroll_empty_slot_mode.label()
            )),
        }
    }

    pub fn set_scroll_empty_mode(&mut self, m: ScrollEmptySlotMode) {
        self.cfg.scroll_empty_slot_mode = m;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::SlotInfo;

    fn slot(index: usize, item: Option<String>, count: u32) -> SlotInfo {
        SlotInfo { index, item, count, max_stack: 64, x: index as f64 * 20.0, y: 0.0, width: 20.0, height: 20.0, enabled: true }
    }

    #[test]
    fn ctrl_disable_passes() {
        let mut c = Controller::new(ModConfig { disable_with_ctrl: true, ..Default::default() });
        let m = GuiModel { cursor_item: None, cursor_count: 0, cursor_max_stack: 64, slots: vec![slot(0, Some("d".into()), 5)] };
        assert_eq!(c.on_scroll(&m, 10.0, 5.0, true, true), ClickAction::Pass);
        assert_eq!(c.on_scroll(&m, 10.0, 5.0, true, false), ClickAction::GiveToCursor { slot: 0, amount: 1 });
    }

    #[test]
    fn release_wrong_button_noop() {
        let mut c = Controller::new(ModConfig::default());
        let m = GuiModel { cursor_item: None, cursor_count: 0, cursor_max_stack: 64, slots: vec![slot(0, Some("d".into()), 5)] };
        c.on_press(&m, 10.0, 5.0, MouseButton::Right, false);
        c.on_move(&m, 10.0, 5.0); // входим в Active
        assert!(c.on_release(&m, 10.0, 5.0, MouseButton::Left).is_empty());
        assert!(!c.on_release(&m, 10.0, 5.0, MouseButton::Right).is_empty());
    }
}
