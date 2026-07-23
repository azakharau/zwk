# zwk

`zwk` is a passive which-key style hint overlay for [Zellij](https://zellij.dev/).
It reads the active Zellij keymap, renders available actions in a bottom-right floating pane, then closes when Zellij returns to the base mode.

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
            MessagePlugin "file:/absolute/path/to/zwk/target/zellij/zellij-which-key-router.wasm" {
                name "zwk:show"
            }
        }
    }

    normal {
        bind "Ctrl Space" "Esc" { SwitchToMode "Locked"; }
        bind "p" { SwitchToMode "Pane"; }
        bind "t" { SwitchToMode "Tab"; }
        bind "s" { SwitchToMode "Scroll"; }
        bind "o" { SwitchToMode "Session"; }
    }

    pane {
        bind "Backspace" { SwitchToMode "Normal"; }
        bind "Ctrl Space" "Esc" { SwitchToMode "Locked"; }
    }
}

plugins {
    which-key-router location="file:/absolute/path/to/zwk/target/zellij/zellij-which-key-router.wasm"
}

load_plugins {
    "file:/absolute/path/to/zwk/target/zellij/zellij-which-key-router.wasm"
}
```

The background instance receives `zwk:show`, creates the menu with its final
size and bottom-right coordinates, then enters Normal mode. Keep the URL
identical in `MessagePlugin`, `plugins`, and `load_plugins`.

See [`examples/zellij-which-key-router.kdl`](examples/zellij-which-key-router.kdl) for a fuller config fragment.

## Permissions

The plugin requests:

- `ReadApplicationState` to inspect the current keymap, panes, tabs, and mode
- `ChangeApplicationState` to position and close the floating pane and change modes
- `OpenTerminalsOrPlugins` to create the menu pane

If Zellij's permission prompt is unreliable, pre-approve the same plugin URL in `permissions.kdl` using Zellij's normal permission-cache format.

## Behavior

- `Ctrl Space` opens the menu in the bottom-right corner without changing pane focus.
- `Backspace` returns from a child mode to the root menu; `Esc` or `Ctrl Space` closes it.
- Hints follow the active Zellij keymap and start with a capital letter.
- The menu is unselectable, so pane actions still target the terminal beneath it.

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
