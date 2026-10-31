//! Стендалон-демо: симуляция сундука 3x9 + хотбара, прогон scripted-событий
//! через тот же контроллер, что использует FFI. Позволяет проверить поведение
//! без запущенной игры.

use mousetweaks_rs::config::ModConfig;
use mousetweaks_rs::controller::Controller;
use mousetweaks_rs::core::{ClickAction, GuiModel, MouseButton, SlotInfo};

fn slot(index: usize, item: Option<&str>, count: u32, x: f64, y: f64) -> SlotInfo {
    SlotInfo {
        index,
        item: item.map(str::to_string),
        count,
        max_stack: 64,
        x,
        y,
        width: 18.0,
        height: 18.0,
        enabled: true,
    }
}

fn chest_demo() -> GuiModel {
    // верхняя часть — сундук (2 ряда по 9), нижняя — хотбар игрока
    let mut slots = Vec::new();
    for row in 0..2 {
        for col in 0..9 {
            let idx = row * 9 + col;
            let item = if idx == 0 { Some("minecraft:dirt") } else { None };
            slots.push(slot(idx, item, if idx == 0 { 64 } else { 0 }, 8.0 + col as f64 * 18.0, 18.0 + row as f64 * 18.0));
        }
    }
    for i in 0..9 {
        slots.push(slot(100 + i, None, 0, 8.0 + i as f64 * 18.0, 120.0));
    }
    GuiModel { cursor_item: None, cursor_count: 0, cursor_max_stack: 64, slots }
}

fn center(model: &GuiModel, index: usize) -> (f64, f64) {
    let s = model.slots.iter().find(|s| s.index == index).unwrap();
    (s.x + s.width / 2.0, s.y + s.height / 2.0)
}

/// Применение результата скролла к модели (эмуляция того, что делает Java-адаптер
/// через ScreenHandler.clickSlot с clickData=amount).
fn apply_scroll(model: &mut GuiModel, a: &ClickAction) {
    match a {
        ClickAction::GiveToCursor { slot, amount } => {
            let s = model.slots.iter_mut().find(|s| s.index == *slot).unwrap();
            s.count -= amount;
            if s.count == 0 {
                s.item = None;
            }
            if model.cursor_item.is_none() {
                model.cursor_item = Some("minecraft:dirt".into());
            }
            model.cursor_count += amount;
        }
        ClickAction::TakeFromCursor { slot, amount } => {
            let s = model.slots.iter_mut().find(|s| s.index == *slot).unwrap();
            s.count += amount;
            if s.item.is_none() {
                s.item = model.cursor_item.clone();
            }
            model.cursor_count -= amount;
            if model.cursor_count == 0 {
                model.cursor_item = None;
            }
        }
        _ => {}
    }
}

fn apply_click(model: &mut GuiModel, a: &ClickAction) {
    match a {
        ClickAction::HalfStack { slot } => {
            if model.cursor_count == 0 {
                return;
            }
            let give = (model.cursor_count + 1) / 2;
            let s = model.slots.iter_mut().find(|s| s.index == *slot).unwrap();
            let give = give.min(s.max_stack - s.count);
            s.count += give;
            if s.item.is_none() {
                s.item = model.cursor_item.clone();
            }
            model.cursor_count -= give;
        }
        ClickAction::WholeStack { slot } => {
            let s = model.slots.iter_mut().find(|s| s.index == *slot).unwrap();
            s.count += model.cursor_count;
            if s.item.is_none() {
                s.item = model.cursor_item.clone();
            }
            model.cursor_count = 0;
            model.cursor_item = None;
        }
        _ => {}
    }
}

fn main() {
    let cfg = ModConfig::load(None);
    println!(
        "MouseTweaks-RS demo v{} | items_per_scroll={}",
        mousetweaks_rs::VERSION,
        cfg.items_per_scroll
    );
    let mut c = Controller::new(cfg);
    let mut model = chest_demo();

    // 1) Скролл «вверх» над стаком земли в слоте 0 — берём по одному в руки
    let (mx, my) = center(&model, 0);
    for _ in 0..5 {
        let a = c.on_scroll(&model, mx, my, true, false);
        apply_scroll(&mut model, &a);
        println!("scroll up   -> {a:?}");
    }
    println!(
        "в руках: {} x{}",
        model.cursor_item.as_deref().unwrap_or("-"),
        model.cursor_count
    );

    // 2) ПКМ-drag по пустым слотам хотбара — раскладка половиной стака
    let (px, py) = center(&model, 100);
    let a = c.on_press(&model, px, py, MouseButton::Right, false);
    println!("press RMB   -> {a:?}");
    for idx in [101usize, 102, 103] {
        let (sx, sy) = center(&model, idx);
        for act in c.on_move(&model, sx, sy) {
            apply_click(&mut model, &act);
            println!("drag move   -> {act:?}");
        }
    }
    let (rx, ry) = center(&model, 104);
    for act in c.on_release(&model, rx, ry, MouseButton::Right) {
        apply_click(&mut model, &act);
        println!("release     -> {act:?}");
    }

    // 3) Попытка выброса стака из рук (Ctrl+Q / double-click)
    println!("toss attempt-> {:?}", c.on_toss(true));

    println!(
        "\nИтог: cursor={} x{}, непустые слоты: {:?}",
        model.cursor_item.as_deref().unwrap_or("-"),
        model.cursor_count,
        model
            .slots
            .iter()
            .filter(|s| s.count > 0)
            .map(|s| (s.index, s.count))
            .collect::<Vec<_>>()
    );
}
