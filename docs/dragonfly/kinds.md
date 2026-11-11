# Kinds: the libraries of Dragonfly

A **kind** is the word before the name of a declaration: `usecase CreateTask`, `controller TasksController`,
`stateless Home`. It tells Dragonfly what that block is, and the dialect writes the whole declaration in the target
(the class, its base, its imports, its boilerplate) so you only write the logic. Kinds are to Dragonfly what
libraries are to a language: a platform comes with its **native kinds**, and a project or the community can add
**extension kinds** on top of a dialect, without touching the compiler (ADR-0095).

The parser never knows a kind. Every kind is "a header and a block" of the base grammar; the kind itself is data:
a TOML entry and a template. That is why adding one needs no lexer, no parser and no Rust.

## 1. Native kinds

The native kinds are the platform's contract: the same words, with the same meaning, in every dialect of the
platform. Change the dialect and the kinds do not change. Kinds differ between platforms, because a backend and a
mobile app are different things.

### Backend (`spec/backend.md`)

| Kind | Where | What it is |
|---|---|---|
| `appConfig` | `config/` | The project's connections (databases, brokers, models) |
| `config` | component `config/` | The connections of one component |
| `entity` | `data/entities/` | A table: `@entity("name")`, fields with `@primary(generated = true)`, `@column(...)`, relations `@manyToOne`, `@oneToMany`, `@manyToMany`, `@oneToOne` |
| `repository` | `data/repositories/` | Queries over an entity; `find`, `findAll`, `save`, `remove` come with the kind |
| `model` | `domain/models/` | What the API reads and answers, with validation decorators |
| `usecase` | `domain/usecases/` | One business operation: `exec(...)` returning a `Result` |
| `service` | `domain/` | Helpers of the use cases (mapping, transformations) |
| `aggregate` | `domain/` | Business rules several use cases share |
| `appcomponent` | component root | The facade: what the rest of the app sees of a component |
| `controller` | `api/` | HTTP routes: `@controller("tasks")`, `@get("{id}")`, `@post()` methods |
| `listener` | `events/` | Methods that receive every message of a topic: `@consumer("topic")` |
| `tools` | `ai/` | Methods a model can call; the doc comment is what the model reads |
| `agent` | `ai/` | An agent with its tools and prompt: `~guide.ask(question)` |
| `state` | `ai/` | The state of a graph: fields read as `state.field` |
| `graph` | `ai/` | A graph of nodes: `@start`, `@next("node")` |
| `tests` | `test/` | Tests: `@test("what it checks")` methods |

### Mobile (`spec/mobile.md`)

| Kind | Where | What it is |
|---|---|---|
| `appConfig`, `config` | `config/` | Connections and globals of the project or of one component |
| `stateless`, `stateful` | `presentation/screens/`, `widgets/` | Widgets; `@screen(route = ...)` makes one a screen |
| `repository` | `data/repositories/` | HTTP calls: `@repository(path)` and `@get`/`@post` methods without body |
| `usecase`, `service`, `aggregate` | `domain/` | The same roles as in backend |
| `appcomponent` | component root | The facade of a component |
| `model` | `domain/models/` | Immutable data with JSON in and out |
| `converter` | `domain/` | How a value type reads and writes JSON |
| `statemanager`, `statemodel` | `presentation/` | The state of a screen and its possible states |
| `form`, `formmanager` | `presentation/*/forms/` | A form's fields and rules, and its validation state |
| `component` | `shared/components/` | A global component; `@on(Event)` methods run on every event |
| `guard` | `shared/guards/` | A custom access check: `canActivate()` |
| `event` | `shared/events/` | A global event with JSON, so another engine can publish it |
| `tests`, `integration` | `test/`, `integration_test/` | Unit and widget tests; tests on a device |
| `tools`, `flow`, `chat`, `remote`, `knowledge` | `ai/` (plugin `genkit`) | AI: tools, flows, chats with memory, remote flows, texts searched by meaning |

A kind that needs an optional part of the framework belongs to a **plugin** and is available when `config.nuc`
enables it (`plugins(genkit = true)`); using it without the plugin is diagnostic N0304, which tells you the line to
add ([README](README.md)).

## 2. Extension kinds: yours and the community's

Anything a project repeats can become a kind: a scheduled job, a webhook handler, a report, a screen layout your
app uses forty times. An extension kind is a **package**: its TOML, templates, the runtime it needs in the target
language, its type definitions and its editor docs, with one implementation per dialect it supports. It is enabled
like a plugin, `plugins(jobs = true)`, and the project keeps `dragonfly(platform = "backend", dialect = "fastapi")`.
A package belongs to one platform: a backend package never shows up in a mobile project, even when the word exists
there with another meaning (`repository` is table queries in backend and HTTP calls in mobile).

**The editor knows the difference.** Completion lists only the kinds of your platform: the native ones plus the
packages your `config.nuc` enables. Hover says where a kind comes from (the platform, or a package and the dialects
it implements) and shows its docs. A kind of a package made for another platform, or not implemented for your
dialect, is reported with the line to fix.

The rule of portability (ADR-0093, ADR-0095): the Nuclear you write with a kind is the same in every dialect that
implements it. A package implemented for one dialect only ties your project to that dialect, exactly as using a
dialect's type definitions does; the compiler tells you which dialects you can still build with.

Status: the package format, `nuclear kind new`, `dragonfly add` and the missing-implementation diagnostic (N0306)
exist (ADR-0095); the editor's view of a kind's source and `nuclear dialect test` are pending (`STATUS.md`).

## 3. Write a kind

A backend project that needs scheduled jobs. One command, two edits, no compiler change.

**1. The package**, created in the project:

```sh
nuclear kind new jobs -p backend -d fastapi
```

```
kinds/jobs/
├── kind.toml                 [package] name, version, platform, description
├── docs.toml                 what the editor shows for each word
└── fastapi/
    ├── kind.toml             [plugin] (runtime packages) and [kinds.job]
    └── templates/job.tpl     the whole declaration in Python
```

**2. The kind**, `kinds/jobs/fastapi/kind.toml`:

```toml
[plugin]
packages = {}

[kinds.job]
description = "job Name(deps) + run() → a scheduled task of the app"
requires = ["run"]            # N0209 if a job has no run()
async_methods = true          # every method is asynchronous in the target
template = { file = "templates/job.tpl" }
```

and its template, `kinds/jobs/fastapi/templates/job.tpl`, which writes the whole declaration (the variables come
from the header and the block; `nuclear spec --dialect fastapi` lists them all):

```
class {{name}}:
    """Scheduled job: {{source_name}}."""

    def __init__(self{{#params_list}}, {{name}}: {{type}}{{/params_list}}) -> None:
{{#params_list}}
        self.{{name}} = {{name}}
{{/params_list}}
{{^params_list}}
        pass
{{/params_list}}

    {{methods_code}}
```

**3. The Nuclear**, with the package enabled in `config.nuc`:

```nuclear
// config.nuc
dragonfly(platform = "backend", dialect = "fastapi")
app(name = "demo")
plugins(jobs = true)
```

```nuclear
// components/cleanup/jobs/purge_job.nuc
job PurgeJob
  run() int
    print("purging")
    0
```

```sh
nuclear build .
```

```python
class PurgeJob:
    """Scheduled job: PurgeJob."""

    def __init__(self) -> None:
        pass

    async def run(self) -> int:
        print("purging")
        return 0
```

Change `dialect` to `nestjs` and the build says what the rule of portability says: the package `jobs` has no
implementation for `nestjs` (it has `fastapi`), so either build with fastapi or add `kinds/jobs/nestjs/` (N0306).
To use the package in every project of your machine: `dragonfly add kinds/jobs`. The editor completes `job`,
shows its `description` and the text of `docs.toml`, and reports a job without `run()` while you type.

### What a kind can declare

| Key | Use |
|---|---|
| `template` | The whole declaration, inline or in a `.tpl` file |
| `requires`, `methods`, `returns` | Methods the block must have; the only methods allowed; the type every method returns |
| `async_methods`, `void_methods` | Every method is asynchronous; methods return nothing |
| `inherits`, `overrides`, `members` | Bases from the type definitions (their members are known), methods that override, inherited names |
| `imports` | Symbols imported when the generated code writes them |
| `rename`, `named_params`, `param_access` | Output name, named constructor arguments, how constructor parameters are read |
| `alias_via`, `provide`, `declares` | Where an alias's instance comes from; how other declarations are read inside the kind; extra classes the template writes |
| `file_header`, `syntax` | Text once per file; `[syntax]` templates that change only inside this kind |

The template receives the declaration's data: `name`, `params_list` (each with `name`, `type`, `tinfo`, `decor`),
`fields`, `methods`, `methods_code`, `members`, `decor` (each decorator with its arguments), `ret_info`, `extends`,
`implements`, `generics`, `file`, `component`, `kind`, and more ([projects.md §5](https://github.com/nuclearlang/nuclear/blob/main/spec/projects.md)). Templates
are a Mustache subset: sections, inverted sections, `@first`/`@last`, and filters such as `pascal`, `snake`, `kebab`,
`join` ([dialects.md §4](https://github.com/nuclearlang/nuclear/blob/main/spec/dialects.md)).

A kind can also bring decorators (`[decorators] consumed`), modifiers and clauses, its own `[auto_imports]` and a
runtime package in the target language; the full format is [dialects.md](https://github.com/nuclearlang/nuclear/blob/main/spec/dialects.md) §5 and §7.

## 4. What a kind cannot do

- **Add syntax.** Inside the block you write Nuclear; the kind chooses what the block becomes, never how Nuclear is
  read. If something needs new grammar, it is not a kind.
- **Touch the compiler.** If a template cannot express what you need, ask for a generic capability (a template
  variable or an `[options]` entry): it is added for every kind, never as a special case. Do not fake the output.
- **Offer a second way of writing the same thing.** A kind exists only if it removes repeated code of the target
  (golden rule, ADR-0038).
- **Generate code an expert would not write:** no reflection, no unused layers, no analyzer warnings.
- **Change with the dialect.** The Nuclear of a kind is the same in every dialect that implements it.

Rules and format: [dialects.md](https://github.com/nuclearlang/nuclear/blob/main/spec/dialects.md) §5.4, §7 and §8, [projects.md](https://github.com/nuclearlang/nuclear/blob/main/spec/projects.md) §5.
Why: ADR-0093, ADR-0095.
