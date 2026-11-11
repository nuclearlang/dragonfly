---
layer: L2
component: dragonfly
read-when: you design something for Dragonfly in any platform, you add a dialect, or you need the rules every platform and dialect must follow
---

# Dragonfly — platforms, dialects and the philosophy

The `dragonfly` repository (ADR-0083, plan 006), with the `dragonfly` CLI ([cli](../cli/CONTEXT.md)).

## What it is

Dragonfly is the framework of NuclearLang: in Nuclear only the business logic; constructors, overrides,
injection, JSON, http, states, modules and imports are written by the transpiler (ADR-0041). It has **three
platforms** (ADR-0093), each a specification written in Nuclear only, and **dialects** grouped under them:

| Platform | What it is | Spec | Dialects (language + framework) |
|---|---|---|---|
| `mobile` | apps for Android, iOS, desktop and web through one UI toolkit | `spec/mobile.md` | `flutter` (Dart) · planned: `react-native`, `kotlin-multiplatform` |
| `backend` | HTTP APIs, data, events and AI agents | `spec/backend.md` | `fastapi` (Python) · `nestjs` (TypeScript) · planned: `express`, Dragonfly's own in Rust |
| `web` | web apps | — (to do) | planned: `react`, `vue`, `angular` |

**The platform is what the user and the agent write.** Its kinds, decorators, folders, `config.nuc`, plugins,
tests and the standard library (`spec/library.md`, ADR-0094) are the same for every dialect of the platform; the
Nuclear code of a project never changes when its dialect changes. The one exception is a dialect's type
definitions (`numpy` only in Python): allowed, and it ties that code to that dialect. Kinds differ between
platforms (`stateless` is mobile, `controller` is backend), never between the dialects of one platform.

A project declares both in `config.nuc`: `dragonfly(platform = "backend", dialect = "fastapi")`; the CLI creates it
with `dragonfly new -p backend -d fastapi <name>`. Folders: `dialects/<platform>/platform.toml` (the contract: kinds,
decorators, plugins, folders, editor docs) and `dialects/<platform>/<dialect>/` (the implementation, which names
its platform in `[dialect]`); the loader refuses a dialect that misses part of its platform. The language dialects
`typescript`, `dart`, `python` (`dialects/<name>/`) are the layer every dialect extends. Today's folders
(`dialects/mobile/flutter`, `dialects/backend/fastapi`, `dialects/backend/nestjs`) move in phase B1 (`STATUS.md`).

## Rules every platform and dialect follows

1. **Light clean architecture.** Components with layers (`config`, `data`, `domain`, and the platform's entry
   layer: `presentation` in mobile, `api` in backend) and a facade (`appcomponent`); the layers exist in the
   generated code, not as ceremony in Nuclear (ADR-0039, 0042).
2. **Closed components.** From another component only its public kinds are visible (facade, models, events,
   global components, guards, screens); the transpiler checks it (N0302). Who-calls-whom rules (N0301): a use
   case only from the facade; a service only from a use case or an aggregate (ADR-0046).
3. **Injection by header.** A typed parameter in a kind's header is injected (ADR-0043).
4. **Errors as values.** Use cases return `Result<E, T>` (ADR-0036, 0047); the runtime's errors carry an HTTP
   status in backend.
5. **Only `.nuc` in the project**; everything generated goes to `build/` and is never edited (ADR-0045, 0050).
6. **Only third-party packages and plugins** in `config.nuc`; the framework's packages are added automatically
   (ADR-0069, ADR-0092: a plugin's packages only when it is enabled).
7. **The same concepts in every platform, implemented natively:** events on demand, session, ACL with guards,
   forms, translations, logs by component (ADR-0052–0060, 0078); connections declared in `config/`.
8. **Warn** when something breaks the architecture or brings security or performance problems
   ([dragonfly-mobile/security.md](https://github.com/nuclearlang/dragonfly-mobile/blob/main/docs/context/dragonfly-mobile/security.md)).
9. **Commands per platform** in the CLI: `dragonfly <command>` reads `dragonfly(...)` (ADR-0067, 0072). The CLI knows
   no language; what it needs is declared by the dialect (`[toolchain]`, planned: today `"flutter"` is hard-coded).
10. Golden rule for every kind, decorator or shortcut (ADR-0038).
11. **A conformance test per platform:** one example built from the same `.nuc` with every dialect of the
    platform (ADR-0093 §9).
12. **Kinds are the libraries** (ADR-0095): native kinds in the platform contract (`dialects/<platform>/platform.toml`);
    extension kinds as packages (`kinds/<name>/`, `spec/dialects.md` §7: `nuclear kind new`, `dragonfly add`,
    `plugins(name = true)` in `config.nuc`, N0306 without an implementation for the dialect), made of TOML,
    templates, a runtime and docs only, never of compiler code. A package belongs to one platform; one implemented
    for some of its dialects ties the project to them. The loader exposes each kind's source
    (`Dialect.kind_sources`: language, platform, package with version and dialects); the editor must list only the
    project's platform kinds and show the source (pending in the LSP). User guide: `docs/dragonfly/kinds.md`.

## Proposals and open questions

Dragonfly items waiting for the user (design, ⟨proposals⟩) are in `STATUS.md` ("Pending: Dragonfly").
A ⟨proposal⟩ is something Claude decided that waits for the user's review; once decided it becomes a rule and
an ADR.

## Where the platforms live

[dragonfly-mobile](https://github.com/nuclearlang/dragonfly-mobile/blob/main/docs/context/dragonfly-mobile/CONTEXT.md) · [dragonfly-backend](https://github.com/nuclearlang/dragonfly-backend/blob/main/docs/context/dragonfly-backend/CONTEXT.md) ·
[dragonfly-web](https://github.com/nuclearlang/dragonfly-web/blob/main/docs/context/dragonfly-web/CONTEXT.md) (ADR-0097: the mobile platform in a browser, dialect `react`). The `[toolchain]` section and dialects installable from a repository:
`specs/002-dialects-with-agents/plan.md` D5–D6.
