# AGENTS.md — dragonfly

Dragonfly, the framework of NuclearLang: the `dragonfly` CLI (new projects, generators, build, run, test, migrate, devices, the editor workspace) and the philosophy shared by its three platforms, mobile, backend and web. Each platform is its own repository: [dragonfly-mobile](https://github.com/nuclearlang/dragonfly-mobile), [dragonfly-backend](https://github.com/nuclearlang/dragonfly-backend), [dragonfly-web](https://github.com/nuclearlang/dragonfly-web).

This repository is a folder of the NuclearLang working tree (`nuclear/dragonfly`); the platform's rules are the
working tree's `../AGENTS.md` ([nuclear](https://github.com/nuclearlang/nuclear/blob/main/AGENTS.md)) and its state is `../STATUS.md`.

- Context of this repository: [docs/context/cli/CONTEXT.md](docs/context/cli/CONTEXT.md).
- What is inside: `dragonfly/` (the CLI crate), `docs/dragonfly/` (the guide, the tutorial, kinds), `docs/context/` (the context for agents and contributors: `dragonfly/` the platforms, `cli/` the CLI).
- Checks:

```sh
cargo test && cargo clippy --all-targets      # here (its own Cargo workspace)
(cd .. && ./install.sh --no-vscode)            # the installed dragonfly and its help
```
