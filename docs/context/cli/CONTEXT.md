---
layer: L2
component: cli
read-when: you touch the dragonfly CLI (this repository's dragonfly/ crate) or spec/flutter.md §11
---

# The `dragonfly` CLI

The `dragonfly` repository (its own Cargo workspace, inside the `nuclear` working tree as `dragonfly/`; plan 006).
Flutter apps written in NuclearLang: it calls `flutter` inside `build/app` and takes from
Flutter only create, run, build, publish and packages.

## Rules

1. The `nuclear` repository's `../crates/*` are used as a library by path
   (`nuclear-transpile::project`, `nuclear-dialect::Loader`, `nuclear-diag`); a missing transpiler feature is
   requested by message.
2. A Dragonfly project has only `.nuc` files; the CLI writes only into `build/` (the app and
   `build/<name>.code-workspace`). Generators never overwrite a file.
3. Done = `cargo test` + `cargo clippy --all-targets` green and `./install.sh --no-vscode` at the root (the user
   runs `~/.local/bin/dragonfly`; the VS Code workspace points to it).
4. A new command or behaviour updates `spec/flutter.md` §11 (and §3.1 for `config.nuc`), an ADR if it is a
   decision, this tree, `docs/dragonfly/tutorial.md`, the `USAGE` of `main.rs` and the skeletons of `scaffold.rs`.
5. Colors, logos and animation only in a terminal; `NO_COLOR` respected; `check --json` through a pipe stays clean.
6. When the dialect changes (`@screen`, kinds, events), update the skeletons and test `new` + `g` + `check` +
   `analyze` + `test`.

## Code map

| File | What it does | L3 |
|---|---|---|
| `dragonfly/src/main.rs` | commands: `new` (n), `make` (g) with aliases, `code`, `watch`, `build`, `run`, `test [--integration]`, `analyze`, `check`, `pub`, `doctor`, `setup`, `repair`, `explain`, `credits`/`author`, gateway to flutter; `find_flutter`/`NO_FLUTTER`; root = folder with `main.nuc` | [build.md](build.md) |
| `dragonfly/src/explain.rs` | `dragonfly explain [--json]`: a Dart/Flutter error on stdin → `title`, `hint` (`HINTS`), `frames` in `.nuc` (line map; also web stacks) | [build.md](build.md) |
| `dragonfly/src/packages.rs` | `config.nuc` sync, third-party definitions, `flutter add/remove` | [packages.md](packages.md) |
| `dragonfly/src/assets.rs` | assets by convention (ADR-0071) | [packages.md](packages.md) |
| `dragonfly/src/scaffold.rs` | generator templates (project, component, screen `--manager`/`--state`, statemanager/statemodel (`presentation`), stateful, usecase, repository, model, service, aggregate, config, test, event, widget, guard, global, page, integration), `snake`/`pascal`; test: every skeleton parses | — |
| `dragonfly/src/vscode.rs` | `build/<name>.code-workspace` | [build.md](build.md) |
| `dragonfly/src/doctor.rs`, `setup.rs` | `doctor` (only NuclearLang/Dragonfly checks), `setup [--dir]` (installs Flutter step by step, asking first) | — |
| `dragonfly/src/watch.rs` | `watch`: polls `.nuc` dates every 250 ms; `dragonfly: transpiling/ready/errors`; exits when its parent dies (Unix) | [build.md](build.md) |
| `dragonfly/src/device/` | Dragonfly devices (ADR-0081) | [devices.md](devices.md) |
| `dragonfly/src/landing.rs` | landing page, animation, header, colors, help | [landing.md](landing.md) |

## Commands

```sh
cargo test && cargo clippy --all-targets
(cd .. && ./install.sh --no-vscode)
dragonfly doctor
dragonfly new shop_app && cd shop_app && dragonfly code
```

## Platforms and the manifest (ADR-0093)

`dragonfly new [-p mobile] [-d flutter] <name>`: the dialect is loaded and must belong to the platform; mobile/flutter
is scaffolded, created and built as before; a backend dialect (`fastapi`, `nestjs`) gets `scaffold::backend_project`
(`config.nuc` + `config/project_config.nuc`) and a web/react project gets `scaffold::web_project` (`config.nuc` + `main.nuc`
with the router + `config/project_config.nuc`); both are built with `nuclear build .` and run through the dialect's
`[toolchain]`.
`build` also writes the project card `.nuclear/project.json` (`project::write_metadata`, spec/projects.md §1.1).
`manifest()` reads `config.nuc` through `Manifest::read` (`dragonfly(platform, dialect)`, `app(name)`); an app without
the line is still flutter (its `dragonfly check` reports N0303 with the line to add); a non-flutter project is
refused with the `nuclear build .` hint. `packages::template` writes the `dragonfly(...)` line first. Dialects live in
`dialects/<platform>/<dialect>/`; the loader finds them by name as before. Added by the platform session.

## Backend projects

`non_mobile_manifest(root)` tells a backend or web project (`dragonfly(platform = "backend"|"web", ...)`) from a mobile
app; for it,
`build` is `nuclear build .` and `install`, `run`, `test` run the dialect's `[toolchain]` command (`Dialect.toolchain`,
`{{packages}}` = `<install>/packages`) with `sh -c` (`cmd /C` on Windows) in the output folder (`backend_command`;
more placeholders through `vars`, quoted by `shell_quote`). `dragonfly migrate [make "<message>" | up | down]` runs
`migrate_make` / `migrate` / `migrate_down` with `{{message}}` and `{{migrations}}` = `<project>/migrations`
(`migrate_command`, ADR-0100 §5). `dragonfly new -p backend`
writes `SQLite(file = env("DATABASE", "app.sqlite"), createTables = true)`, `plugins(sqlite = true)` and a first
test in `test/app_test.nuc` (`scaffold::backend_project`). `dragonfly new -p web -d react` writes `main.nuc` (the App
with `createElement(RouterProvider, router = appRouter())`) and an `Http` connection (`scaffold::web_project`); add
screens with `dragonfly make screen`. `dragonfly check` on a backend or web project delegates to `nuclear check .`
(the Flutter check resolves a pubspec and fonts they do not have).

## Kind packages (ADR-0095)

`dragonfly add <folder>` installs a kind package (`kinds/<name>/` with `kind.toml` `[package]`) in `<install>/kinds/`
after loading it with every dialect it implements (`add_package`, `copy_dir` in `main.rs`); `loader()` adds
`kinds/` of the current folder and `<install>/kinds` to `Loader.packages`. The skeleton comes from `nuclear kind
new <name> -p <platform> -d <dialect>` (crates/nuclear-cli). Pending: installing from a repository with a pinned
version (plan 002 D6).

## Plugins

`build` adds the packages of the plugins `config.nuc` enables (`plugin_packages`, `project::enabled_plugins`) and
removes those of plugins turned off (`sync_plugin_packages`, `build/app/.dragonfly_plugins`); a `local` package is
found with `runtime_package` (installed `packages/`, else the repository). Added by the platform session (ADR-0092).

## Known limits

- Generators normalize with `snake()` but do not validate identifiers (leading digit, reserved words).
- `--org` only in `new`; when `build` creates a missing app it uses `com.example`.
- Dialects and `packages/dragonfly` are found in the installation (`nuclear_dialect::install_dir()`), else in
  the repository where it was built. The `dragonfly` dependency is a local path until it is on pub.dev.
- Flutter apps are built here with the Flutter commands; backend and web projects are built with `nuclear build .` and
  run/install/test/check through the dialect's `[toolchain]` (`manifest()` still refuses non-flutter for the Flutter-only
  paths). A fresh web project builds and type-checks (`tsc`), but `vite build`/`dragonfly run` need the react runtime's
  bundling fix (Pending: Dragonfly web): rollup cannot resolve `react` from the `--install-links`ed `dragonfly-web`.
