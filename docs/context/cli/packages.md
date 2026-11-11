---
layer: L3
component: cli
read-when: you change config.nuc handling, dragonfly flutter add/remove, generated definitions or assets
---

# Packages, definitions and assets

## `packages.rs` (ADR-0065–0070)

- `read` (sections from the dialect's `[project.manifest]`), `sync` (`app`/`environment` fields and packages →
  `build/app/pubspec.yaml` by editing lines; `flutter pub add/remove`; `build/app/.dragonfly_deps` = packages
  that come from config.nuc, so only those are removed; never removes Dragonfly's own, `OWN` + `PACKAGES`).
- `load_defs`: version from `pubspec.lock` → `~/.cache/nuclearlang/defs/dart/<p>/<v>`; if missing,
  `generate_defs` runs `dart run bin/defs.dart` of `tools/defs` with `--exclude` of the dialect's packages;
  `Loader::add_defs`; writes `build/defs.lock`; skips packages the dialect knows.
- `add`/`remove` (`dragonfly flutter add|remove`, also `pub add|remove`): edit config.nuc with
  `set_entry`/`remove_entry`, one entry per line; `add` rejects reserved packages
  (`project::reserved_package`) before running flutter; `sync` never adds a reserved one.
- `ensure_config`: `check` and `build` create config.nuc if missing (name, description/version, third-party
  packages of the app; those with a map, `path:`/`git:`, are left out). `template`: config.nuc of `new`.
  `manifest()` takes the name from `app(name)`. `check` only uses cached definitions.

Limits: only sections written one entry per line can be edited (a one-line section with entries errors,
`block`); the pubspec is edited by lines (`set_yaml`/`yaml_get`), not with a YAML parser; versions are text
(`"^2.2.3"`), no `path:`/`git:` yet. A package without `lib/<name>.dart` is described with every file of
`lib/`; types re-exported from another project package repeat (first wins). `devDependencies` have no
definitions. `generate_defs` prints a long command. `flutter add` runs two builds.

## `assets.rs` (ADR-0071)

`write` rewrites `assets:`/`fonts:`/`shaders:` under `flutter:` of the app's pubspec on every build (the rest is
kept; keys recognized by 2-space indentation); `fonts` groups `assets/fonts/<Family>/` by the last word of the
name and errors when two files match (`build` and `check` call it before writing). Tested with `flutter build
bundle` (FontManifest and shader).
