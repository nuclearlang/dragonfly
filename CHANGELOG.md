# Changelog

All notable changes to dragonfly are written here; versions follow [Semantic Versioning](https://semver.org).

## Unreleased

- First public version.
- `dragonfly gen inspect|component|resource` (ADR-0103): a resource's finished code from `--fields`, `--from-json` or `--from-spec`, for the three platforms.
- `dragonfly gen usecase <component> <Name> [--calls <Repository>.<method>] [--route "POST orders/{id}/cancel"]` (ADR-0103): a use case wired into its component (its alias in the facade; backend: its route in the controller of the route's prefix), its signature read from the project; with `--calls` the use case is written too. A `merge` piece adds whole members (a route keeps its decorator and its body), and a name another file declares also stops a new `merge` file.
- `docs/dragonfly/generators.md`: the generators' guide (every option, what each platform gets, a use case written and wired step by step, the refusals, the MCP tool of each command, what is not generated yet).
- `dragonfly gen resource` on a resource that exists finishes what it adds: a new action also gets the repository's method and the controller's route (it left a use case calling a method nobody had). `--source db --actions search` is refused: which column is searched is a decision (a query of the repository, wired with `gen usecase --calls`). ADR-0105.
