use std::collections::BTreeMap;

use zellij_tile::prelude::actions::{Action, SearchDirection};
use zellij_tile::prelude::*;

const MIN_MENU_WIDTH: usize = 24;
const MENU_MARGIN_RIGHT: usize = 2;
const FLOATING_FRAME_COLUMNS: usize = 2;
const FLOATING_FRAME_ROWS: usize = 2;
const KEY_TEXT_PADDING: usize = 2;
const CELL_GAP: usize = 2;
const ANSI_RESET: &str = "\x1b[0m";
const ANSI_KEY: &str = "\x1b[7m";
const SELF_PLUGIN: &str = "which-key-router";
const STALE_MENU_SECONDS: u64 = 5;

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[derive(Default)]
struct WhichKeyHints {
    is_menu: bool,
    mode_info: ModeInfo,
    active_rows: usize,
    active_cols: usize,
    permissions_granted: bool,
    menu_pane: Option<PaneId>,
    show_requested: bool,
    saw_nonbase: bool,
}

#[cfg(target_arch = "wasm32")]
impl ZellijPlugin for WhichKeyHints {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.is_menu = configuration.get("zwk-role").map(String::as_str) == Some("menu");
        if self.is_menu {
            let spawned = configuration
                .get("zwk-spawn")
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(0);
            if now_epoch_s().saturating_sub(spawned) > STALE_MENU_SECONDS {
                close_self();
                return;
            }
        }

        request_hint_permissions();
        subscribe(&[
            EventType::ModeUpdate,
            EventType::TabUpdate,
            EventType::PaneUpdate,
            EventType::PermissionRequestResult,
        ]);
        set_selectable(false);
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::ModeUpdate(mode_info) => {
                self.mode_info = mode_info;
                if !self.is_menu {
                    self.try_open_menu();
                    return false;
                }

                if menu_should_close(&self.mode_info, &mut self.saw_nonbase) {
                    close_self();
                    return false;
                }
                self.resize_to_keymap();
                true
            }
            Event::TabUpdate(tabs) => {
                if let Some(tab) = tabs.iter().find(|tab| tab.active) {
                    self.active_rows = tab.viewport_rows;
                    self.active_cols = tab.viewport_columns;
                }
                if self.is_menu {
                    self.resize_to_keymap();
                } else {
                    self.try_open_menu();
                }
                self.is_menu
            }
            Event::PaneUpdate(manifest) => {
                if !self.is_menu {
                    if let Some(PaneId::Plugin(menu_id)) = self.menu_pane {
                        let alive = manifest
                            .panes
                            .values()
                            .flatten()
                            .any(|pane| pane.is_plugin && pane.id == menu_id);
                        if !alive {
                            self.menu_pane = None;
                        }
                    }
                    self.try_open_menu();
                }
                false
            }
            Event::PermissionRequestResult(PermissionStatus::Granted) => {
                self.permissions_granted = true;
                set_selectable(false);
                if self.is_menu {
                    switch_to_input_mode(&InputMode::Normal);
                } else {
                    self.try_open_menu();
                }
                self.is_menu
            }
            Event::PermissionRequestResult(PermissionStatus::Denied) => self.is_menu,
            _ => false,
        }
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        if self.is_menu || pipe_message.name != "zwk:show" {
            return false;
        }
        self.show_requested = true;
        self.try_open_menu();
        false
    }

    fn render(&mut self, rows: usize, cols: usize) {
        if !self.is_menu || self.mode_info.base_mode.is_none() {
            print!("\x1b[2J\x1b[H");
            return;
        }
        let keymap = self.effective_keymap();
        print!(
            "{}",
            render_overlay(&keymap, self.permissions_granted, rows, cols)
        );
    }
}

#[cfg(target_arch = "wasm32")]
impl WhichKeyHints {
    fn try_open_menu(&mut self) {
        if !self.show_requested || self.menu_pane.is_some() || !self.permissions_granted {
            return;
        }

        let keymap = self.mode_info.get_keybinds_for_mode(InputMode::Normal);
        let Some((width, height)) = self.dimensions_for(&keymap) else {
            return;
        };
        let x = self
            .active_cols
            .saturating_sub(width.saturating_add(MENU_MARGIN_RIGHT));
        let y = bottom_aligned_y(self.active_rows, height);
        let mut coordinates = FloatingPaneCoordinates::default()
            .with_x_fixed(x)
            .with_y_fixed(y)
            .with_width_fixed(width)
            .with_height_fixed(height);
        coordinates.pinned = Some(true);
        coordinates.borderless = Some(true);

        let mut config = BTreeMap::new();
        config.insert("zwk-role".to_owned(), "menu".to_owned());
        config.insert("zwk-spawn".to_owned(), now_epoch_s().to_string());

        if let Some(pane_id) =
            open_plugin_pane_floating(SELF_PLUGIN, config, Some(coordinates), BTreeMap::new())
        {
            self.menu_pane = Some(pane_id);
            self.show_requested = false;
        }
    }

    fn resize_to_keymap(&self) {
        if self.mode_info.base_mode.is_none() {
            return;
        }
        let keymap = self.effective_keymap();
        let Some((width, height)) = self.dimensions_for(&keymap) else {
            return;
        };
        let x = self
            .active_cols
            .saturating_sub(width.saturating_add(MENU_MARGIN_RIGHT));
        let y = bottom_aligned_y(self.active_rows, height);
        let mut coordinates = FloatingPaneCoordinates::default()
            .with_x_fixed(x)
            .with_y_fixed(y)
            .with_width_fixed(width)
            .with_height_fixed(height);
        coordinates.pinned = Some(true);
        coordinates.borderless = Some(true);
        let own_pane = PaneId::Plugin(get_plugin_ids().plugin_id);
        change_floating_panes_coordinates(vec![(own_pane, coordinates)]);
    }

    fn effective_keymap(&self) -> Vec<(KeyWithModifier, Vec<Action>)> {
        if self.saw_nonbase {
            get_keymap_for_mode(&self.mode_info)
        } else {
            self.mode_info.get_keybinds_for_mode(InputMode::Normal)
        }
    }
    fn dimensions_for(&self, keymap: &[(KeyWithModifier, Vec<Action>)]) -> Option<(usize, usize)> {
        if self.active_cols == 0 || self.active_rows == 0 {
            return None;
        }
        let rows = overlay_rows(keymap, self.permissions_granted);
        let content_height = overlay_content_height(rows.len());
        let content_width = overlay_content_width(&rows);
        floating_pane_dimensions(
            self.active_cols,
            self.active_rows,
            content_width,
            content_height,
        )
    }
}

fn request_hint_permissions() {
    request_permission(&[
        PermissionType::ReadApplicationState,
        PermissionType::ChangeApplicationState,
        PermissionType::OpenTerminalsOrPlugins,
    ]);
}

fn get_keymap_for_mode(mode_info: &ModeInfo) -> Vec<(KeyWithModifier, Vec<Action>)> {
    match mode_info.mode {
        InputMode::Normal => mode_info.get_keybinds_for_mode(InputMode::Normal),
        InputMode::Pane => mode_info.get_keybinds_for_mode(InputMode::Pane),
        InputMode::Tab => mode_info.get_keybinds_for_mode(InputMode::Tab),
        InputMode::Resize => mode_info.get_keybinds_for_mode(InputMode::Resize),
        InputMode::Move => mode_info.get_keybinds_for_mode(InputMode::Move),
        InputMode::Scroll => mode_info.get_keybinds_for_mode(InputMode::Scroll),
        InputMode::Search => mode_info.get_keybinds_for_mode(InputMode::Search),
        InputMode::Session => mode_info.get_keybinds_for_mode(InputMode::Session),
        _ => mode_info.get_mode_keybinds(),
    }
}

fn render_overlay(
    keymap: &[(KeyWithModifier, Vec<Action>)],
    permissions_granted: bool,
    rows: usize,
    cols: usize,
) -> String {
    if rows < FLOATING_FRAME_ROWS || cols < FLOATING_FRAME_COLUMNS {
        return "\x1b[2J\x1b[H".to_owned();
    }

    let rows_to_render = overlay_rows(keymap, permissions_granted);
    let key_column_width = key_column_width(&rows_to_render);
    let inner_rows = rows.saturating_sub(FLOATING_FRAME_ROWS);
    let inner_cols = cols.saturating_sub(FLOATING_FRAME_COLUMNS);
    let row_capacity = inner_rows;
    let mut output = String::from("\x1b[2J\x1b[H");
    let truncated = rows_to_render.len() > row_capacity;
    let visible_rows = if truncated {
        row_capacity.saturating_sub(1)
    } else {
        row_capacity
    };

    output.push_str(&border_top(cols));
    for (key, description) in rows_to_render.iter().take(visible_rows) {
        output.push('\n');
        output.push('│');
        output.push_str(&render_hint_line(
            key,
            description,
            key_column_width,
            inner_cols,
        ));
        output.push('│');
    }
    if truncated && row_capacity > 0 {
        let hidden_rows = rows_to_render.len().saturating_sub(visible_rows);
        output.push('\n');
        output.push('│');
        output.push_str(&render_hint_line(
            "…",
            &format!("More: {hidden_rows}"),
            key_column_width,
            inner_cols,
        ));
        output.push('│');
    }
    output.push('\n');
    output.push_str(&border_bottom(cols));

    output
}

fn border_top(cols: usize) -> String {
    format!(
        "┌{}┐",
        "─".repeat(cols.saturating_sub(FLOATING_FRAME_COLUMNS))
    )
}

fn border_bottom(cols: usize) -> String {
    format!(
        "└{}┘",
        "─".repeat(cols.saturating_sub(FLOATING_FRAME_COLUMNS))
    )
}

fn overlay_content_height(row_count: usize) -> usize {
    row_count
}

fn render_hint_line(key: &str, description: &str, key_column_width: usize, cols: usize) -> String {
    let button = key_button(key);
    let button_width = button.chars().count();
    let key_padding = key_column_width.saturating_sub(button_width);
    let description_width = cols.saturating_sub(key_column_width.saturating_add(CELL_GAP));

    let description = fit_plain(description, description_width);
    let visible_width = key_column_width
        .saturating_add(CELL_GAP)
        .saturating_add(description.chars().count());
    let line_padding = cols.saturating_sub(visible_width);

    format!(
        "{ANSI_KEY}{button}{ANSI_RESET}{}{}{}{}",
        " ".repeat(key_padding),
        " ".repeat(CELL_GAP),
        description,
        " ".repeat(line_padding)
    )
}

fn floating_pane_dimensions(
    active_cols: usize,
    active_rows: usize,
    content_width: usize,
    content_height: usize,
) -> Option<(usize, usize)> {
    let available_width = active_cols.saturating_sub(MENU_MARGIN_RIGHT);
    let available_content_width = available_width.saturating_sub(FLOATING_FRAME_COLUMNS);
    let available_content_height = active_rows.saturating_sub(FLOATING_FRAME_ROWS);

    if available_width == 0 || available_content_width == 0 || available_content_height == 0 {
        return None;
    }

    let content_width = content_width
        .max(MIN_MENU_WIDTH)
        .min(available_content_width);
    let content_height = content_height.max(1).min(available_content_height);
    let pane_width = content_width.saturating_add(FLOATING_FRAME_COLUMNS);
    let pane_height = content_height.saturating_add(FLOATING_FRAME_ROWS);

    Some((pane_width, pane_height))
}

fn bottom_aligned_y(active_rows: usize, pane_height: usize) -> usize {
    active_rows.saturating_sub(pane_height)
}

fn should_render_for_mode(mode_info: &ModeInfo) -> bool {
    mode_info.base_mode.is_some() && Some(mode_info.mode) != mode_info.base_mode
}

fn menu_should_close(mode_info: &ModeInfo, saw_nonbase: &mut bool) -> bool {
    if mode_info.base_mode.is_none() {
        return false;
    }
    if should_render_for_mode(mode_info) {
        *saw_nonbase = true;
        return false;
    }
    *saw_nonbase
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn now_epoch_s() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn overlay_rows(
    keymap: &[(KeyWithModifier, Vec<Action>)],
    permissions_granted: bool,
) -> Vec<(String, String)> {
    if !permissions_granted {
        return vec![
            ("!".to_owned(), "Waiting for Zellij permissions".to_owned()),
            (
                "↵".to_owned(),
                "Approve prompt or cache permissions".to_owned(),
            ),
        ];
    }

    let mut rows = keymap
        .iter()
        .filter_map(|(key, actions)| {
            action_sequence_label(actions).map(|label| (key.to_string(), label))
        })
        .collect::<Vec<_>>();
    sort_overlay_rows(&mut rows);
    if rows.is_empty() {
        rows.push(("?".to_owned(), "No supported keybinds".to_owned()));
    }
    rows
}

fn sort_overlay_rows(rows: &mut [(String, String)]) {
    rows.sort_by_key(|(key, _)| key_sort_value(key));
}

fn key_sort_value(key: &str) -> (u8, u8, String) {
    let normalized = key.trim().to_ascii_lowercase();
    let first = normalized.chars().next();
    let category = match first {
        Some(ch) if ch.is_ascii_digit() => 0,
        Some(ch) if ch.is_ascii_alphabetic() => 1,
        _ => 2,
    };
    let digit = first
        .and_then(|ch| ch.to_digit(10))
        .map(|digit| digit as u8)
        .unwrap_or(u8::MAX);

    (category, digit, normalized)
}

fn key_button(key: &str) -> String {
    format!(" {key} ")
}

fn key_column_width(rows: &[(String, String)]) -> usize {
    rows.iter()
        .map(|(key, _)| key.chars().count().saturating_add(KEY_TEXT_PADDING))
        .max()
        .unwrap_or(KEY_TEXT_PADDING)
}

fn overlay_content_width(rows: &[(String, String)]) -> usize {
    let key_width = key_column_width(rows);
    let description_width = rows
        .iter()
        .map(|(_, description)| description.chars().count())
        .max()
        .unwrap_or(0);

    key_width
        .saturating_add(CELL_GAP)
        .saturating_add(description_width)
}

fn fit_plain(text: &str, cols: usize) -> String {
    if cols == 0 {
        return String::new();
    }
    text.chars().take(cols).collect()
}

fn mode_label(mode: InputMode) -> &'static str {
    match mode {
        InputMode::Normal => "Normal",
        InputMode::Locked => "Locked",
        InputMode::Resize => "Resize",
        InputMode::Pane => "Pane",
        InputMode::Tab => "Tab",
        InputMode::Scroll => "Scroll",
        InputMode::EnterSearch => "Enter search",
        InputMode::Search => "Search",
        InputMode::RenameTab => "Rename tab",
        InputMode::RenamePane => "Rename pane",
        InputMode::Session => "Session",
        InputMode::Move => "Move",
        InputMode::Prompt => "Prompt",
        InputMode::Tmux => "Tmux",
    }
}

fn action_sequence_label(actions: &[Action]) -> Option<String> {
    let labels = actions
        .iter()
        .filter(|action| !matches!(action, Action::SwitchToMode { input_mode } if *input_mode == InputMode::Locked))
        .filter_map(action_label)
        .collect::<Vec<_>>();

    if labels.is_empty() {
        None
    } else {
        Some(capitalize(&labels.join(" → ")))
    }
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    first.to_uppercase().chain(chars).collect()
}

fn action_label(action: &Action) -> Option<String> {
    match action {
        Action::SwitchToMode { input_mode } => Some(format!("{} mode", mode_label(*input_mode))),
        Action::SwitchModeForAllClients { input_mode } => {
            Some(format!("all clients: {} mode", mode_label(*input_mode)))
        }
        Action::SwitchFocus => Some("switch focus".to_owned()),
        Action::MoveFocus { direction } => Some(format!("focus {}", direction_word(*direction))),
        Action::MoveFocusOrTab { direction } => {
            Some(format!("focus/tab {}", direction_word(*direction)))
        }
        Action::MovePane { direction } => Some(match direction {
            Some(direction) => format!("move pane {}", direction_word(*direction)),
            None => "move pane".to_owned(),
        }),
        Action::MovePaneBackwards => Some("move pane back".to_owned()),
        Action::NewPane { direction, .. } | Action::NewTiledPane { direction, .. } => {
            Some(match direction {
                Some(direction) => format!("split {}", direction_word(*direction)),
                None => "new pane".to_owned(),
            })
        }
        Action::NewFloatingPane { .. } => Some("new floating pane".to_owned()),
        Action::NewInPlacePane { .. } => Some("new in-place pane".to_owned()),
        Action::NewBlockingPane { .. } => Some("new blocking pane".to_owned()),
        Action::NewStackedPane { .. } => Some("new stacked pane".to_owned()),
        Action::CloseFocus => Some("close pane".to_owned()),
        Action::ToggleFocusFullscreen => Some("fullscreen".to_owned()),
        Action::ToggleFloatingPanes => Some("toggle floating panes".to_owned()),
        Action::HideFloatingPanes { .. } => Some("close overlay".to_owned()),
        Action::ShowFloatingPanes { .. } => Some("show floating panes".to_owned()),
        Action::TogglePaneEmbedOrFloating => Some("embed/float pane".to_owned()),
        Action::TogglePanePinned => Some("toggle pane pinned".to_owned()),
        Action::StackPanes { .. } => Some("stack panes".to_owned()),
        Action::TogglePaneInGroup => Some("toggle pane group".to_owned()),
        Action::ToggleGroupMarking => Some("toggle group marking".to_owned()),
        Action::TogglePaneFrames => Some("toggle pane frames".to_owned()),
        Action::Resize { resize, direction } => Some(resize_label(*resize, *direction)),
        Action::FocusNextPane => Some("next pane".to_owned()),
        Action::FocusPreviousPane => Some("previous pane".to_owned()),
        Action::NewTab { .. } => Some("new tab".to_owned()),
        Action::CloseTab => Some("close tab".to_owned()),
        Action::GoToNextTab => Some("next tab".to_owned()),
        Action::GoToPreviousTab => Some("previous tab".to_owned()),
        Action::GoToTab { index } => Some(format!("go to tab {index}")),
        Action::ToggleTab => Some("toggle tab".to_owned()),
        Action::ToggleActiveSyncTab => Some("toggle sync".to_owned()),
        Action::MoveTab { direction } => Some(format!("move tab {}", direction_word(*direction))),
        Action::BreakPane => Some("break pane".to_owned()),
        Action::BreakPaneLeft => Some("break pane left".to_owned()),
        Action::BreakPaneRight => Some("break pane right".to_owned()),
        Action::PaneNameInput { .. } => Some("rename pane".to_owned()),
        Action::TabNameInput { .. } => Some("rename tab".to_owned()),
        Action::UndoRenamePane => Some("undo pane rename".to_owned()),
        Action::UndoRenameTab => Some("undo tab rename".to_owned()),
        Action::ScrollUp => Some("scroll up".to_owned()),
        Action::ScrollDown => Some("scroll down".to_owned()),
        Action::ScrollToTop => Some("scroll top".to_owned()),
        Action::ScrollToBottom => Some("scroll bottom".to_owned()),
        Action::PageScrollUp => Some("page up".to_owned()),
        Action::PageScrollDown => Some("page down".to_owned()),
        Action::HalfPageScrollUp => Some("half page up".to_owned()),
        Action::HalfPageScrollDown => Some("half page down".to_owned()),
        Action::EditScrollback { .. } => Some("edit scrollback".to_owned()),
        Action::Search { direction } => Some(match direction {
            SearchDirection::Up => "search up".to_owned(),
            SearchDirection::Down => "search down".to_owned(),
        }),
        Action::SearchInput { .. } => Some("search input".to_owned()),
        Action::SearchToggleOption { .. } => Some("toggle search option".to_owned()),
        Action::Detach => Some("detach".to_owned()),
        Action::Quit => Some("quit".to_owned()),
        Action::LaunchOrFocusPlugin { plugin, .. }
        | Action::LaunchPlugin { plugin, .. }
        | Action::NewFloatingPluginPane { plugin, .. }
        | Action::NewTiledPluginPane { plugin, .. }
        | Action::NewInPlacePluginPane { plugin, .. }
        | Action::StartOrReloadPlugin { plugin } => {
            Some(plugin_location_label(&plugin.location_string()))
        }
        Action::KeybindPipe { name, payload, .. } | Action::CliPipe { name, payload, .. } => {
            let pipe_name = name.as_deref().or(payload.as_deref()).unwrap_or("pipe");
            Some(pipe_name.replace(['_', '-'], " "))
        }
        Action::NoOp => None,
        _ => None,
    }
}

fn plugin_location_label(location: &str) -> String {
    let without_scheme = location.strip_prefix("file:").unwrap_or(location);
    let file_name = without_scheme.rsplit('/').next().unwrap_or(without_scheme);
    let name = file_name.strip_suffix(".wasm").unwrap_or(file_name);

    name.replace(['-', '_'], " ")
}

fn resize_label(resize: Resize, direction: Option<Direction>) -> String {
    let verb = match resize {
        Resize::Increase => "increase",
        Resize::Decrease => "decrease",
    };
    match direction {
        Some(direction) => format!("{verb} {}", direction_word(direction)),
        None => format!("{verb} size"),
    }
}

fn direction_word(direction: Direction) -> &'static str {
    match direction {
        Direction::Left => "left",
        Direction::Right => "right",
        Direction::Up => "up",
        Direction::Down => "down",
    }
}

#[cfg(target_arch = "wasm32")]
register_plugin!(WhichKeyHints);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_common_actions() {
        assert_eq!(
            action_sequence_label(&[Action::SwitchToMode {
                input_mode: InputMode::Pane,
            }])
            .as_deref(),
            Some("Pane mode")
        );
        assert_eq!(
            action_sequence_label(&[Action::MoveFocus {
                direction: Direction::Left,
            }])
            .as_deref(),
            Some("Focus left")
        );
        assert_eq!(
            action_sequence_label(&[Action::GoToTab { index: 3 }]).as_deref(),
            Some("Go to tab 3")
        );
        assert_eq!(
            action_sequence_label(&[Action::NewStackedPane {
                command: None,
                pane_name: None,
                near_current_pane: false,
                tab_id: None,
            }])
            .as_deref(),
            Some("New stacked pane")
        );
    }

    #[test]
    fn labels_action_sequences_vertically() {
        assert_eq!(
            action_sequence_label(&[
                Action::NewPane {
                    direction: Some(Direction::Right),
                    pane_name: None,
                    start_suppressed: false,
                },
                Action::SwitchToMode {
                    input_mode: InputMode::Normal,
                },
            ])
            .as_deref(),
            Some("Split right → Normal mode")
        );
    }

    #[test]
    fn rows_keep_key_and_description_separate() {
        let rows = overlay_rows(
            &[(
                KeyWithModifier::new(BareKey::Char('p')),
                vec![Action::SwitchToMode {
                    input_mode: InputMode::Pane,
                }],
            )],
            true,
        );

        assert_eq!(rows, vec![("p".to_owned(), "Pane mode".to_owned())]);
        assert_eq!(key_button("p"), " p ");
        let content_width = overlay_content_width(&rows);
        assert_eq!(
            render_hint_line("p", "Pane mode", key_column_width(&rows), content_width),
            "\x1b[7m p \x1b[0m  Pane mode"
        );
    }

    #[test]
    fn locked_cleanup_action_is_not_rendered_as_primary_hint() {
        assert_eq!(
            action_sequence_label(&[
                Action::SwitchToMode {
                    input_mode: InputMode::Locked,
                },
                Action::GoToTab { index: 1 },
            ])
            .as_deref(),
            Some("Go to tab 1")
        );
        assert_eq!(
            action_sequence_label(&[Action::SwitchToMode {
                input_mode: InputMode::Locked,
            }]),
            None
        );
    }

    #[test]
    fn rows_sort_digits_before_letters_and_hide_close_only_bindings() {
        let rows = overlay_rows(
            &[
                (KeyWithModifier::new(BareKey::Char('b')), vec![Action::Quit]),
                (
                    KeyWithModifier::new(BareKey::Char('2')),
                    vec![Action::GoToTab { index: 2 }],
                ),
                (
                    KeyWithModifier::new(BareKey::Char('a')),
                    vec![Action::SwitchToMode {
                        input_mode: InputMode::Pane,
                    }],
                ),
                (
                    KeyWithModifier::new(BareKey::Char('1')),
                    vec![Action::GoToTab { index: 1 }],
                ),
                (
                    KeyWithModifier::new(BareKey::Char('x')),
                    vec![Action::SwitchToMode {
                        input_mode: InputMode::Locked,
                    }],
                ),
            ],
            true,
        );

        assert_eq!(
            rows.iter().map(|(key, _)| key.as_str()).collect::<Vec<_>>(),
            vec!["1", "2", "a", "b"]
        );
        assert!(
            rows.iter()
                .all(|(_, hint)| hint.chars().next().is_some_and(char::is_uppercase))
        );
    }

    #[test]
    fn overlay_dimensions_fit_visible_content() {
        let rows = vec![
            ("p".to_owned(), "Pane mode".to_owned()),
            ("Ctrl SPACE".to_owned(), "close overlay".to_owned()),
        ];
        let content_width = overlay_content_width(&rows);

        assert_eq!(
            key_column_width(&rows),
            "Ctrl SPACE".chars().count() + KEY_TEXT_PADDING
        );
        assert_eq!(
            floating_pane_dimensions(120, 40, content_width, rows.len()),
            Some((
                content_width + FLOATING_FRAME_COLUMNS,
                rows.len() + FLOATING_FRAME_ROWS
            ))
        );
        assert_eq!(bottom_aligned_y(40, rows.len() + FLOATING_FRAME_ROWS), 36);
    }

    #[test]
    fn base_mode_controls_overlay_visibility_like_zjstatus_hints() {
        let locked_mode = ModeInfo {
            mode: InputMode::Locked,
            base_mode: Some(InputMode::Locked),
            ..Default::default()
        };
        let normal_mode = ModeInfo {
            mode: InputMode::Normal,
            base_mode: Some(InputMode::Locked),
            ..Default::default()
        };
        let pane_mode = ModeInfo {
            mode: InputMode::Pane,
            base_mode: Some(InputMode::Locked),
            ..Default::default()
        };

        assert!(
            !should_render_for_mode(&locked_mode),
            "base mode renders nothing — the pane is hidden outright"
        );
        assert!(should_render_for_mode(&normal_mode));
        assert!(should_render_for_mode(&pane_mode));
    }

    #[test]
    fn menu_opens_before_mode_change_and_closes_on_return_to_base() {
        let locked_mode = ModeInfo {
            mode: InputMode::Locked,
            base_mode: Some(InputMode::Locked),
            ..Default::default()
        };
        let normal_mode = ModeInfo {
            mode: InputMode::Normal,
            base_mode: Some(InputMode::Locked),
            ..Default::default()
        };
        let mut saw_nonbase = false;

        assert!(!menu_should_close(&locked_mode, &mut saw_nonbase));
        assert!(!menu_should_close(&normal_mode, &mut saw_nonbase));
        assert!(menu_should_close(&locked_mode, &mut saw_nonbase));
    }

    #[test]
    fn denied_permissions_show_help_rows() {
        let rows = overlay_rows(&[], false);

        assert!(rows.iter().any(|(_, label)| label.contains("permissions")));
        assert!(
            rows.iter()
                .all(|(_, hint)| hint.chars().next().is_some_and(char::is_uppercase))
        );
    }

    #[test]
    fn truncated_menu_reports_hidden_rows() {
        let keymap = vec![
            (KeyWithModifier::new(BareKey::Char('a')), vec![Action::Quit]),
            (KeyWithModifier::new(BareKey::Char('b')), vec![Action::Quit]),
            (KeyWithModifier::new(BareKey::Char('c')), vec![Action::Quit]),
        ];

        let output = render_overlay(&keymap, true, 4, 30);

        assert!(output.contains("More: 2"));
        assert!(!output.contains(" b "));
    }
}
