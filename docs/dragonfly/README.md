# Dragonfly: three platforms, one code per platform

Dragonfly is the framework of NuclearLang. You write a **platform** — `mobile`, `backend` or `web` — in Nuclear:
entities, use cases, screens, endpoints, agents. A **dialect** is the language and framework it comes out in, and
the Nuclear code of a project never changes when its dialect changes: a backend written once builds as a Python
service or as a TypeScript service from the same files. Only a use of a dialect's type definitions (`numpy` exists
in Python only) ties code to one dialect.

| Platform | Dialect | What comes out | Guide | Rules |
|---|---|---|---|---|
| `mobile` | `flutter` | Dart, Flutter, bloc, go_router, json_serializable | [Tutorial](tutorial.md) | [mobile.md](https://github.com/nuclearlang/dragonfly-mobile/blob/main/spec/mobile.md), [flutter.md](https://github.com/nuclearlang/dragonfly-mobile/blob/main/spec/flutter.md) |
| `mobile` + plugin `genkit` | `flutter` | Genkit Dart: chats with tools, flows, RAG, remote flows | [Genkit](https://github.com/nuclearlang/dragonfly-mobile/blob/main/docs/dragonfly/genkit.md) | [flutter.md §6.20](https://github.com/nuclearlang/dragonfly-mobile/blob/main/spec/flutter.md) |
| `backend` | `fastapi` (Python) | FastAPI, SQLAlchemy 2 async, pydantic, LangChain, LangGraph | [Python](https://github.com/nuclearlang/dragonfly-backend/blob/main/docs/dragonfly/python.md) | [backend.md](https://github.com/nuclearlang/dragonfly-backend/blob/main/spec/backend.md), [fastapi.md](https://github.com/nuclearlang/dragonfly-backend/blob/main/spec/fastapi.md) |
| `backend` | `nestjs` (TypeScript) | NestJS 12, Sequelize, class-validator, LangChain.js, LangGraph.js | [NestJS](https://github.com/nuclearlang/dragonfly-backend/blob/main/docs/dragonfly/nestjs.md) | [backend.md](https://github.com/nuclearlang/dragonfly-backend/blob/main/spec/backend.md), [nestjs.md](https://github.com/nuclearlang/dragonfly-backend/blob/main/spec/nestjs.md) |
| `web` | `react` (TypeScript) | React 19 (no JSX), React Router 7, Vite, Vitest | [React](https://github.com/nuclearlang/dragonfly-web/blob/main/docs/dragonfly/react.md) | [web.md](https://github.com/nuclearlang/dragonfly-web/blob/main/spec/web.md), [react.md](https://github.com/nuclearlang/dragonfly-web/blob/main/spec/react.md) |

The platforms share the same architecture, so moving from one to another means learning its kinds, not a new
way to think:

- **Components** by feature (`components/tasks/`), each with `data/` (entities, repositories), `domain/` (models,
  use cases) and its entry points (`presentation/` in Flutter, `api/` in the backends, `ai/` for agents).
- **Kinds** say what a declaration is: `usecase`, `repository`, `controller`, `stateless`, `agent`, `graph`...
  The dialect writes the class, its decorators, its registration in the injector and its imports.
- **`appcomponent`** is what the rest of the app sees of a component; one component never reaches inside another.
- **Errors are values:** a use case answers `Result<Error, T>`; a controller turns `Err(NotFound(...))` into a 404.
- **Connections** (a database, a chat model, Kafka, Redis, an AI provider) are listed in
  `config/project_config.nuc`, never opened by hand.
- **Tests in Nuclear:** `tests Name` with `@test(...)` methods, written for the target's test runner.

## Contracts: one backend, its apps generated

A backend's build publishes what it exposes (`.nuclear/contract.json`, plus OpenAPI); an app declares it
(`contracts(tasks_api = "../tasks_api")` or a url) and its models and repositories are generated at build time, rules
and keys included; a renamed field breaks the app's build instead of the phone (ADR-0098). Guide:
[Contracts](https://github.com/nuclearlang/dragonfly-backend/blob/main/docs/dragonfly/contracts.md).

## Mobile: the backend's ORM on the device

A mobile app keeps what it fetches with two words (ADR-0096). `@primary` on a field gives the model an encrypted
local table (`@entity("characters")` names it; the same database on phones, desktops and web); `saveLocal = true` on the repository makes every GET that answers that model save its rows and get a
twin `<method>AsLocal()`, same signature, that answers the last rows from the device without a request. The
queries are the backend repository's: `where`, `orderBy`, `limit`, `offset`, `getOne`, `getMany`, `count`,
`exists`, `update`, `delete`, plus `find`, `findAll`, `save`, `remove`.

```nuclear
@entity("characters")
Character
  @primary
  id: int
  name: string
  status: string

@repository(path = "character", saveLocal = true)
repository CharacterRepository
  @get(path = ":id")
  getOne(id: int) Result<CharacterError, Character>     // + getOneAsLocal(id)

  alive() Character[]
    ~local<Character>().where({ "status": "Alive" }).orderBy("name").getMany()
```

Encryption (AES-256, key in the device's secure storage) is on by default and is a line of the config files:
`local: LocalDatabase = LocalDatabase(encrypted = true)` in the project's `appConfig`, or in a component's `config`
for its own tables.

Rules and the generated Dart: [flutter.md §6.3](https://github.com/nuclearlang/dragonfly-mobile/blob/main/spec/flutter.md); the package API: [dragonfly.md
§3.2](https://github.com/nuclearlang/dragonfly-mobile/blob/main/spec/dragonfly.md).

## Plugins: only what the project uses

The optional parts of each framework are **plugins**, enabled in the project's `config.nuc`. A project without a
plugin does not carry its packages:

```nuclear
// config.nuc
plugins(genkit = true, googleAI = true)
```

| Dialect | Plugins |
|---|---|
| `flutter` | `genkit`, and its providers `firebaseAI`, `googleAI`, `vertexAI`, `openAI`, `anthropic` |
| `fastapi`, `nestjs` (the backend platform: the same list) | `sqlite`, `postgres`, `mysql`, `mariadb`, `redis`, `kafka`, `events`, `ai`, `openai`, `ollama`, `chroma` |

- Only the enabled plugins reach `pubspec.yaml`, `pyproject.toml` or `package.json`. Turning one off removes its
  packages on the next build.
- A provider plugin enables the plugin it needs (`googleAI` enables `genkit`; `ollama` enables `ai`).
- Using a kind or a class of a plugin that is not enabled is diagnostic **N0304**, which says which line to add.
  The editor still completes those names, so you can discover them.

How a dialect declares its plugins: [projects.md §3.3](https://github.com/nuclearlang/nuclear/blob/main/spec/projects.md). Why: ADR-0092.

The native kinds of each platform, and how to write your own kind without touching the compiler: [Kinds](kinds.md).

## Generators: the finished code of a resource

`dragonfly gen resource <component> <Name>` writes, for any platform, the models, the repository (REST over an `Http`
connection or a database), one use case per action, the component's facade, the controller (backend) or the screen
with its state manager (mobile, web) and their tests; everything compiles as written. The fields come from
`--fields "id:int name:string"`, from an answer (`--from-json <url|file>`: nested models and pages included, `@key`
for snake_case keys) or from a spec (`--from-spec spec.md`: its example answer, its `Key | Type` table, its
`GET <url>` line). `dragonfly gen inspect` lists what the project has; nothing is ever overwritten.

`dragonfly gen usecase <component> <Name>` wires a use case into its component: its alias in the facade and, on the
backend, `--route "POST orders/{id}/cancel"` adds its route to the controller of `orders` (made when missing). A use
case with rules of its own is written first and then wired, with the signature the project gives it; one that only
calls a repository method is written by the command too: `--calls OrderRepository.pending`.

Every option, what each platform gets, the refusals and worked examples: [Generators](generators.md). The MCP server
gives the same tools to an agent, and the editor's Loop runs them before planning. Why: ADR-0103.

## Writing with an AI agent

Each dialect has a short sheet for agents: `nuclear spec --brief --dialect fastapi` (also `nestjs`, `flutter`),
and `nuclear help <word> --dialect <d>` explains a kind or decorator. `nuclear agent init --dialect <d>` writes
the `AGENTS.md` of a project.
