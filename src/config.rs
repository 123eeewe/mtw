//! Конфигурация мода (аналог config/ папки MouseTweaks).
//!
//! Файл: `<корень игры>/config/mousetweaks_rs.toml` (путь передаёт хост-адаптер
//! из Java-части, обычно `FabricLoader.getInstance().getConfigDir()`).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Режим поведения при наведении на пустой слот.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScrollEmptySlotMode {
    /// Скроллить предметы в соседнем стеке того же предмета, если он есть.
    #[default]
    NearestSameItemStack,
    /// Пустые слоты игнорируются (можно пролистывать «через» них).
    Ignore,
    /// Скролл полностью блокируется, если под курсором пустой слот.
    Disable,
}

impl ScrollEmptySlotMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::NearestSameItemStack => "Ближайший стек того же предмета",
            Self::Ignore => "Игнорировать",
            Self::Disable => "Отключено",
        }
    }
}

/// Полный набор настроек (совпадает с оригинальным MouseTweaks + расширения).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModConfig {
    /// Главный переключатель всех возможностей.
    pub enable: bool,
    /// Отключать ли возможности мода, когда зажат Ctrl.
    pub disable_with_ctrl: bool,
    /// Сколько предметов passing'ом за один «щелчок» колеса мыши.
    /// 0 — полстака, -1 — весь стак.
    pub items_per_scroll: i32,
    /// Инвертировать направление скролла.
    pub reverse_scroll_direction: bool,
    /// Разрешить перетаскивание левым кликом (drag-splitting).
    pub allow_left_dragging: bool,
    /// Разрешить перетаскивание правым кликом.
    pub allow_right_dragging: bool,
    /// Поведение при наведении на пустой слот.
    pub scroll_empty_slot_mode: ScrollEmptySlotMode,
    /// Отключать ли анимацию «броска стака» (Ctrl+Drop / двойной клик) —
    /// вместо выброса всего стака из рук предмет остаётся в руках.
    pub disable_toss_enabled: bool,
    /// Показывать HUD-подсказку о текущих зажатых кнопках во время drag.
    pub show_hud_overlay: bool,
    /// Включить «безопасный режим»: не трогать чужие GUI (торговые экраны и т.п.).
    pub safe_mode: bool,
}

impl Default for ModConfig {
    fn default() -> Self {
        Self {
            enable: true,
            disable_with_ctrl: false,
            items_per_scroll: 1,
            reverse_scroll_direction: false,
            allow_left_dragging: true,
            allow_right_dragging: true,
            scroll_empty_slot_mode: ScrollEmptySlotMode::default(),
            disable_toss_enabled: true,
            show_hud_overlay: true,
            safe_mode: true,
        }
    }
}

pub const MOD_ID: &str = "mousetweaks_rs";

fn config_path(root: Option<&Path>) -> PathBuf {
    let base = root.unwrap_or(Path::new("."));
    base.join("config").join(format!("{MOD_ID}.toml"))
}

impl ModConfig {
    /// Загрузка конфигурации; при отсутствии файла — создание дефолтного.
    pub fn load(root: Option<&Path>) -> Self {
        let path = config_path(root);
        match fs::read_to_string(&path) {
            Ok(s) => toml::from_str(&s).unwrap_or_else(|e| {
                eprintln!("[{MOD_ID}] битый конфиг ({}), беру дефолт: {e}", path.display());
                Self::default()
            }),
            Err(_) => {
                let cfg = Self::default();
                // best-effort: создадим файл с комментариями
                let _ = fs::create_dir_all(path.parent().unwrap_or(Path::new(".")));
                let _ = fs::write(&path, cfg.to_toml_with_comments());
                cfg
            }
        }
    }

    pub fn save(&self, root: Option<&Path>) -> std::io::Result<()> {
        let path = config_path(root);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(&path, self.to_toml_with_comments())
    }

    /// TOML с человекочитаемыми комментариями (как в game settings у оригинала).
    pub fn to_toml_with_comments(&self) -> String {
        let body = toml::to_string_pretty(self).unwrap_or_default();
        format!(
            "# MouseTweaks-RS (Horizon edition)\n\
             # enable            — главный переключатель\n\
             # items_per_scroll  — предметов за щелчок колеса (0 = полстака, -1 = весь стак)\n\
             # scroll_empty_slot_mode — nearest_same_item_stack | ignore | disable\n\
             # disable_toss_enabled   — отключает «выброс стака» из рук (Ctrl+Q / double-click)\n\n\
             {body}"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let cfg = ModConfig {
            items_per_scroll: -1,
            scroll_empty_slot_mode: ScrollEmptySlotMode::Ignore,
            ..Default::default()
        };
        let s = cfg.to_toml_with_comments();
        let parsed: ModConfig = toml::from_str(&s).unwrap();
        assert_eq!(parsed, cfg);
    }

    #[test]
    fn defaults_when_missing_fields() {
        let parsed: ModConfig = toml::from_str("items_per_scroll = 5").unwrap();
        assert_eq!(parsed.items_per_scroll, 5);
        assert!(parsed.enable);
    }
}
