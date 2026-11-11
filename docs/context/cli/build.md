---
layer: L3
component: cli
read-when: you change dragonfly build/run/test/watch/repair/explain, the markers in .dart_tool, or the VS Code workspace
---

# Build pipeline

`dragonfly build` leaves the app ready to compile and repeats nothing without changes (~40 ms):

1. Creates the app if missing; syncs `config.nuc` → pubspec and loads definitions BEFORE transpiling
   ([packages.md](packages.md)).
2. Transpiles the project; writes only what changed and deletes orphans (`.dart_tool/nuclear_files`).
3. `ensure_packages` (Dragonfly's own; with `@splash` also `flutter_native_splash`, detected by line prefix
   `name:`), copies `out.copies`, `assets::write`.
4. `pub get` when `.dart_tool/package_config.json` is missing (after `flutter clean`) or the pubspec is newer
   than the lock AND than `.dart_tool/nuclear_pub_get` (`pub get` does not touch an unchanged lock).
5. `build_runner --delete-conflicting-outputs` only if a file with `part` changed since
   `.dart_tool/nuclear_build_runner` (`.g.dart` dates are useless: unchanged ones are not rewritten).
6. `flutter_native_splash:create` when the yaml or an image changed (`.dart_tool/nuclear_splash`). Removing
   `@splash` deletes the yaml but leaves the native splash (`flutter_native_splash:remove` needs the yaml: run
   it by hand first).
7. `write_line_map` → `build/app/.dart_tool/nuclear_lines.json` (`BuiltFile::lines`, for debuggers, ADR-0068).

- Errors in `.nuc` → exit code 2 (`CODE_ERRORS`; editors do not repair). `run`, `test` and devices use `--no-pub`.
- `repair`: build+analyze → markers deleted + pub get + build → flutter clean + build, each step only if the
  previous left `flutter analyze` with errors. Editors run it only when something fails.
- `explain`: maps a Dart error to its `.nuc` line; `--json` for editors.
- `watch` rebuilds the whole project and does not see dialect or defs changes (restart it).
- Flutter is found in `FLUTTER`, `FLUTTER_ROOT`, `.fvm/flutter_sdk` (project or above), the PATH, `~/fvm/default`
  and typical folders (`find_flutter`); `NO_FLUTTER` explains where it looked.

## VS Code workspace (`vscode.rs`)

`build/<name>.code-workspace`: Nuclear folder + Flutter app (the Flutter extension treats it as a normal
project: devices, F5, hot reload, DevTools), tasks `dragonfly: watch/build/test/analyze` (no problem patterns:
the LSP gives the errors; errors of closed `.nuc` files only show in the task terminal), launch
debug/profile/release with `preLaunchTask`, `dart.flutterSdkPath`. **Do not rename `"<name> (debug)"` nor its
`type: "dart"`**: the VS Code extension shows ▷ Run/Debug only if it exists and launches it by name. The tasks'
`command` is `current_exe()` (with `cargo run` it points to `target/debug`). Reload on save relies on
`dart.previewHotReloadOnSaveWatcher` (may cause a double reload with the extension's own; can be removed).
