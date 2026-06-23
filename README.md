# zwk

`zwk` is a passive which-key style hint overlay for [Zellij](https://zellij.dev/).
It reads the active Zellij keymap, renders available actions in a small floating pane, then hides itself when Zellij returns to the base mode.

This is not an interactive router. It does not intercept keys or run actions for you.

## Status

- Target Zellij version: `0.44.3`
- Tested locally on macOS
- Plugin artifact: `target/zellij/zellij-which-key-router.wasm`

## Requirements

- Rust with the `wasm32-wasip1` target
- Zellij `0.44.3`

```sh
rustup target add wasm32-wasip1
```

## Build

```sh
./scripts/build-plugin
```

The build script compiles the release WASM target and copies it to:

```text
target/zellij/zellij-which-key-router.wasm
```

## Zellij configuration

Use an absolute `file:` URL in your Zellij config. Replace `/absolute/path/to/zwk` with this repository path.

```kdl
keybinds clear-defaults=true {
    locked {
        bind "Ctrl Space" {
            LaunchOrFocusPlugin "file:/absolute/path/to/zwk/target/zellij/zellij-which-key-router.wasm" {
                floating true
                borderless true
                move_to_focused_tab true
            }
            SwitchToMode "Normal";
            FocusPreviousPane;
        }
    }

    normal {
        bind "Ctrl Space" "Esc" { SwitchToMode "Locked"; }
        bind "p" { SwitchToMode "Pane"; }
        bind "t" { SwitchToMode "Tab"; }
        bind "s" { SwitchToMode "Scroll"; }
        bind "o" { SwitchToMode "Session"; }
    }
}

plugins {
    which-key-router location="file:/absolute/path/to/zwk/target/zellij/zellij-which-key-router.wasm"
}
```

See [`examples/zellij-which-key-router.kdl`](examples/zellij-which-key-router.kdl) for a fuller config fragment.

## Permissions

The plugin requests:

- `ReadApplicationState` to inspect the current keymap, panes, tabs, and mode
- `ChangeApplicationState` to position/hide the floating pane and restore focus to the terminal pane

If Zellij's permission prompt is unreliable, pre-approve the same plugin URL in `permissions.kdl` using Zellij's normal permission-cache format.

## Behavior

- `Ctrl Space` launches or focuses the floating plugin pane.
- The plugin switches Zellij to `Normal` mode so normal-mode bindings are visible.
- The terminal pane is focused again immediately.
- When Zellij returns to its base mode, the plugin hides itself instead of closing, so the next open can reuse the positioned floating pane.
- The plugin does not use `skip_plugin_cache` in the example config.

## Tests

Fast Rust checks:

```sh
cargo fmt --check
cargo test
```

Build the plugin:

```sh
./scripts/build-plugin
```

## Repository layout

```text
src/main.rs                         Zellij plugin implementation
examples/zellij-which-key-router.kdl Config fragment for local installation
scripts/build-plugin                Release WASM build helper
```

## License

MIT. See [`LICENSE`](LICENSE).
