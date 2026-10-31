//! MouseTweaks-RS — аналог мода MouseTweaks для Minecraft-платформы Horizon.
//!
//! Архитектура:
//!  * [`core`]      — чистая логика (scroll-hand, drag-splitting, toss-cancel),
//!                    без зависимостей от игры; покрыта юнит-тестами.
//!  * [`config`]    — TOML-конфиг (`config/mousetweaks_rs.toml`).
//!  * [`controller`]- оркестратор с состоянием drag.
//!  * [`ffi`]       — C-экспорт для Java-адаптера Horizon (JNI/Project Panama).
//!  * [`win_hook`]  — опциональный Win32 LL-хук для стендалон-отладки.
//!
//! Java-адаптер (в составе мод-пака Horizon) вешает миксины на
//! `HandledScreen#mouseScrolled`, `#mouseClicked`, `#init` и дергает native-функции.

pub mod config;
pub mod controller;
pub mod core;
pub mod ffi;
pub mod win_hook;

pub use config::{ModConfig, ScrollEmptySlotMode};
pub use controller::Controller;
pub use core::{ClickAction, DragState, GuiModel, MouseButton, SlotInfo};

/// Версия для fabric.mod.json / мода Horizon.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
