# Contributing

Thanks for improving `zwk`.

## Local checks

Run these before opening a pull request:

```sh
cargo fmt --check
cargo test
./scripts/build-plugin
```

Interactive terminal capture harnesses are intentionally not part of this repository.

## Scope

Keep the plugin passive:

- render hints from Zellij's current keymap
- never take focus from the terminal pane
- close the floating menu when returning to base mode

Avoid turning this plugin into a key router or controller unless the project explicitly changes direction.

## Compatibility

The crate pins `zellij-tile = 0.44.3`; update docs, examples, and tests together when changing the Zellij target version.
