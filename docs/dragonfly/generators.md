# Generators: the standard code, written by a command

Most of a feature is the same every time: a model, a repository, a use case per action, the facade's aliases, the
routes or the screen, the tests. `dragonfly gen` writes that part, finished and compiling, on the three platforms.
A person or an agent writes only what is particular to the app: a use case with rules of its own, the details of a
view.

`dragonfly make` is the other command that creates files: one nearly empty declaration of a kind, to fill in.
`dragonfly gen` writes code that is already done.

| Command | What it writes |
|---|---|
| `dragonfly gen inspect` | nothing: what the project has (platform, dialect, components, every declaration by kind), as JSON |
| `dragonfly gen component <component>` | a component: its facade and its config |
| `dragonfly gen resource <component> <Name>` | a resource: models, repository, a use case per action, the facade's aliases, the controller (backend) or the screen (mobile, web), the tests |
| `dragonfly gen usecase <component> <Name>` | a use case wired into its component: its alias in the facade and, on the backend, its route |

Every command runs inside a project and takes `--dry-run`, which shows the files and writes nothing.

## 1. A resource

```sh
dragonfly gen resource characters Character --from-json https://rickandmortyapi.com/api/character
dragonfly gen resource orders Order --fields "id:int total:double status:string" --source db
dragonfly gen resource episodes Episode --from-spec specs/episodes/spec.md --screen episode_list
```

The fields come from one of three places:

| Option | The fields are read from |
|---|---|
| `--from-json <file\|url>` | an example answer of the API. A nested object becomes a model named `<Name><Key>`, a paged answer gives `<Name>Page`, a snake_case key gets `@key`. A url also gives the API to call |
| `--from-spec <spec.md>` | the spec's example answer, or else its `Key \| Type \| Description` table; a `GET https://...` line gives the API |
| `--fields "id:int name:string origin:Origin?"` | the command line. A type that is not a basic one must be declared in the project |

The other options:

| Option | Meaning |
|---|---|
| `--source rest:<base url>` \| `rest` \| `db` | where the data lives: another API (an HTTP repository) or the database (backend only). Default: `db`, or `rest` when a url was given |
| `--actions list,get,search,create,update,delete` | one use case each. Default: `list,get` for an API, every one for a database |
| `--path <path>` | the API's path of the resource. Default: the url's last segment, or the name |
| `--screen <screen>` | mobile and web: a screen that lists the resource, with its state manager and state model |
| `--plural <Names>` | when the English plural is not the right one |

What a resource is made of on each platform:

| | Backend | Mobile and web |
|---|---|---|
| Data | `rest`: an HTTP repository over an `Http` connection. `db`: an entity, a `New<Name>` model and a paged query | an HTTP repository over the app's `Http` connection |
| Domain | one use case per action | one use case per action |
| Facade | an alias per use case | an alias per use case |
| Entry | the controller with a route per action | with `--screen`, the screen with its state manager |
| Tests | for a database resource | with the repository mocked |

## 2. A use case

A resource brings the standard use cases. `gen usecase` is for the next one, and it covers two different cases.

**A use case that only calls a repository method** is written by the command, because there is nothing to decide:

```sh
dragonfly gen usecase orders PageOrders --calls OrderRepository.page --route "GET orders/pages/{page}"
```

```
    created  components/orders/domain/usecases/page_orders.nuc
   added to  components/orders/orders_appcomponent.nuc
   added to  components/orders/api/orders_controller.nuc
```

```nuclear
// What OrderRepository.page answers, for the rest of orders.
usecase PageOrders(repository: OrderRepository)
  exec(page: int, size: int) Result<Error, Page<Order>>
    Ok(~repository.page(page, size))
```

The parameters and the answer are the method's own, read from the project. An answer that is not a `Result` goes
inside an `Ok` (backend).

**A use case with rules of its own** is written first, by hand or by an agent, because its rules are the part no
command can know. Then the command wires it:

```nuclear
// Cancels an order that is still pending; Conflict (409) when it is not.
usecase CancelOrder(repository: OrderRepository)
  exec(id: int) Result<Error, Order>
    found = ~repository.find(id)
    if found.status != "pending"
      return Err(Conflict("only a pending order is cancelled"))
    found.status = "cancelled"
    Ok(~repository.save(found))
```

```sh
dragonfly gen usecase orders CancelOrder --route "POST orders/{id}/cancel"
```

It adds the alias to the facade and the route to the controller, with the signature `exec` has in the project:

```nuclear
appcomponent OrdersAppComponent
  cancelOrder() = CancelOrder.exec
```

```nuclear
@controller("orders")
controller OrdersController(app: OrdersAppComponent)
  @post("{id}/cancel")
  cancelOrder(id: int) Result<Error, Order>
    ~app.cancelOrder(id)
```

`--route "<VERB> <prefix>[/<path>]"` is backend only. The first segment is the controller's prefix: `orders` is
`OrdersController`, created when the component has none. A `{name}` of the path is a parameter of the use case; the
other parameters arrive as the verb sends them. The route calls the use case with `~` only when its `exec` awaits.

On mobile and web the command adds the alias (and, with `--calls`, writes the use case). What a screen shows of it,
a method of its state manager and the state it emits, is written by hand.

## 3. What every generator keeps

- **Nothing is overwritten.** A file that exists is kept. The facade, the repository and the controller get only the
  members they lack, each one whole (a route with its decorator and its body). So an action added to a resource that
  exists (`dragonfly gen resource orders Order ... --actions list,get,search` on one that had `list,get`) adds its use
  case, its alias, the repository's method and its route, and what is written still compiles. Running a command twice changes nothing: the second
  time every file says `kept`.
- **Nothing is declared twice.** A name another file of the project declares stops the command before it writes.
- **Nothing is left to fill in.** What a command writes compiles as written. When it cannot write something finished,
  it refuses and says what to do.

## 4. When a generator refuses

| It says | What to do |
|---|---|
| `<Name> is not declared. A use case with rules of its own is written first ...` | write the use case in the file it names, then run the command again; or give `--calls` when it only calls a repository |
| the repository, or its method, is not declared | `--calls` names a method a repository of the project has: `dragonfly gen inspect` lists the repositories and their files |
| a `{name}` of the route is not a parameter of the use case | name the path's parameter like the one `exec` takes |
| the use case belongs to another component | give the component that declares it |
| the platform has no routes | `--route` is backend only: on mobile and web leave it out |
| a type of `--fields` is not declared | declare the model first, or take the fields from an answer (`--from-json`), which writes the nested models too |
| the name is declared in another file | pick another name, or remove the old declaration when it is a leftover |
| a database resource has no generated search | which column is searched is yours to say: write the query in the repository, then `dragonfly gen usecase <component> Search<Names> --calls <Name>Repository.<method>` |

## 5. From an AI agent and from the editor

The MCP server of NuclearLang gives the same commands to Claude Code, Codex or Cursor as tools, and tells every model
to generate first and write only what is left:
[the MCP server](https://github.com/nuclearlang/nuclear/blob/main/docs/ai/mcp.md).

| Command | MCP tool |
|---|---|
| `dragonfly gen inspect` | `project_inspect` |
| `dragonfly gen component` | `generate_component` |
| `dragonfly gen resource` | `generate_resource` (and `api_inspect`, which shows an API's shape first) |
| `dragonfly gen usecase` | `generate_usecase` |

The editor's Loop has a *Generate* stage before any agent works: it runs `dragonfly gen resource` for every resource
the spec names, and the planner plans only what is left
([the AI loop](https://github.com/nuclearlang/nuclear/blob/main/docs/ai/loop.md)).

## 6. What is not generated yet

- A database resource has no relations to other entities, and no search: add them by hand.
- A backend resource over another API (`rest`) gets no tests.
- On mobile and web, what a screen shows of a use case added with `gen usecase`.
- The Loop's *Generate* stage writes resources only: it does not call `gen usecase`.

How a platform declares its generators (`[[generators.<what>]]` in its `platform.toml`: templates, never code in the
CLI) is in [the dialects' spec](https://github.com/nuclearlang/nuclear/blob/main/spec/dialects.md). Why they exist:
[ADR-0103](https://github.com/nuclearlang/nuclear/blob/main/docs/adr/0103-generators.md).
