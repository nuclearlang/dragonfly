# dragonfly (CLI)

Creates and works with Flutter apps written in NuclearLang: only `.nuc` files; the Flutter app is generated in
`build/app`. Full rules: [spec/flutter.md §11](https://github.com/nuclearlang/dragonfly-mobile/blob/main/spec/flutter.md) (`nuclear help flutter`).

**Start with the tutorial:** [docs/dragonfly/tutorial.md](docs/dragonfly/tutorial.md) (`nuclear help
tutorial`): an app from scratch with a component, a repository, a screen and tests.

## Install

```sh
cd <NuclearLang repository>
./install.sh                 # nuclear and dragonfly in ~/.local/bin + the VS Code extension
dragonfly doctor             # checks everything is there: CLIs, dialect, dragonfly package, Flutter, VS Code
dragonfly setup              # if Flutter is missing: installs it step by step (system packages, SDK, PATH)
```

`./install.sh --uninstall` removes it. Flutter is found by itself (PATH, fvm, `~/development/flutter`...);
`FLUTTER` or `FLUTTER_ROOT` choose another one.

## Diagnosis and Flutter

- `dragonfly doctor`: checks what NuclearLang and Dragonfly need (like `flutter doctor`): `dragonfly` and
  `nuclear`, the flutter dialect and its type definitions, the dragonfly Dart package, Flutter (where it is and
  whether it is missing from the PATH), VS Code with the NuclearLang, Dart and Flutter extensions, and the
  current project. Each problem comes with how to solve it. Flutter's own (Android, iOS, devices):
  `dragonfly flutter doctor`.
- `dragonfly setup`: installs Flutter following the official guide, asking before each step: system packages
  (apt, dnf, pacman, zypper or Apple's tools), the latest stable version checked with its sha256, unpacking into
  `~/develop` (`--dir` chooses another folder) and your shell's PATH.

## Landing page and credits

- `dragonfly` alone: the first time, an animation (the explosion, WELCOME, everything grows back and the
  dragonfly arrives) and the page with the logos; afterwards, only the page.
- `dragonfly credits` (or `author`): always the full animation, with the author and the repository.
- After WELCOME the motto appears: "Before, everything was chaos; now everything is simpler with Dragonfly" (in
  Spanish if your system is in Spanish).
- Every other command starts with a header with the NuclearLang and Dragonfly logos, and its messages are in
  colors: green = done, red = error, yellow = warning, blue = commands you can type.
- `dragonfly help` (or `-h`): the header and the commands with their symbols and colors. Through a pipe, or with
  `NO_COLOR`, everything comes out as plain text.

## Use

```sh
dragonfly                         # landing page
dragonfly new shop_app            # only .nuc + the Flutter app in build/app
cd shop_app
dragonfly code                    # VS Code: ▷ on a .nuc or F5 runs the app; saving a .nuc reloads it
dragonfly g c products            # component
dragonfly gen resource characters Character --from-json https://rickandmortyapi.com/api/character  # finished code from an API
dragonfly gen usecase characters CharacterEpisodes --calls CharacterRepository.episodes   # a use case over a repository method, wired
dragonfly g s products detail --route /detail -m DetailManager -st DetailView
dragonfly test
dragonfly help                    # every command
```
