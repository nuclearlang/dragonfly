//! `dragonfly create` and `dragonfly make`: the skeleton files of a project and of each kind.
//! Every file is NuclearLang; names are given in snake_case (`user_profile`) and written in PascalCase
//! where the language asks for it (`UserProfileScreen`).

use std::path::{Path, PathBuf};

/// `user_profile` → `UserProfile`.
pub fn pascal(snake: &str) -> String {
    snake
        .split(['_', '-', ' '])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        })
        .collect()
}

/// `UserProfile` or `user-profile` → `user_profile`.
pub fn snake(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 && !out.ends_with('_') {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else if c == '-' || c == ' ' {
            out.push('_');
        } else {
            out.push(c);
        }
    }
    out
}

fn fill(template: &str, vars: &[(&str, &str)]) -> String {
    vars.iter().fold(template.to_string(), |t, (k, v)| t.replace(&format!("{{{k}}}"), v))
}

/// The files of one generator, relative to the project root.
pub type Files = Vec<(PathBuf, String)>;

const MAIN: &str = r#"main() void
  ~startDragonfly()
  runApp(App())

stateless App
  build(context: BuildContext) Widget
    MaterialApp.router(
      title = "{Name}",
      routerConfig = appRouter,
      builder = (context, child) => NetworkWrapper(child = child!),
    )
"#;

const PROJECT_CONFIG: &str = r#"// Global configuration: the connections and the services of the whole app.
appConfig ProjectConfig
  connections: Connection[] = [
    Http(
      baseUrl = "https://api.example.com/",
      connectTimeout = Duration(seconds = 10),
      receiveTimeout = Duration(seconds = 10),
      interceptors = [SessionInterceptor(), HttpLogInterceptor()],
    ),
  ]
  globals: Global[] = [
    Events(),
    SessionManager(timeout = Duration(minutes = 30)),
    NetworkManager(),
  ]
"#;

const COMPONENT: &str = r#"// Facade of the {name} component: the only part other components can use.
appcomponent {Name}Component
  hello() = Hello{Name}UseCase.exec
"#;

const HELLO_USECASE: &str = r#"usecase Hello{Name}UseCase
  exec() Result<string, string[]>
    Ok(["Hello from {name}"])
"#;

const STATE: &str = r#"// Each field is a state of the screen; a boolean state carries no data.
{State}
  (items: string[], error: string, loading: boolean)
"#;

const MANAGER: &str = r#"@stateModel({State})
statemanager {Manager}
  (component: {Name}Component)
    load()

  @state({State}.loading)
  loading()

  @before(loading)
  @state({State}.items)
  load() Result<string, string[]>
    component.hello()
"#;

const SCREEN: &str = r#"@screen(route = "{route}", providers = [{Manager}])
stateless {Screen}Screen
  build(context: BuildContext) Widget
    Scaffold(
      appBar = AppBar(title = Text("{Screen}")),
      body = {State}Builder<{Manager}>(
        loading = () => Center(child = CircularProgressIndicator()),
        items = (items) => ListView(children = items.map(i => ListTile(title = Text(i))).toList()),
        error = (message) => Center(child = Text(message)),
      ),
    )
"#;

const USECASE: &str = r#"usecase {Item}UseCase
  exec(id: int) Result<string, string>
    Ok("{item} ${id}")
"#;

const REPOSITORY: &str = r#"// Each method without a body is an http call of the connection in config/project_config.nuc.
@repository(path = "{item}")
repository {Item}Repository
  @get(path = ":id")
  getOne(id: int) Result<string, {Item}>

  @get(cache = true)
  getAll(page: int) Result<string, {Item}[]>
"#;

const MODEL: &str = r#"{Item}
  id: int
  name: string
"#;

const SERVICE: &str = r#"{kind} {Item}
  run(value: string) string
    value
"#;

const EVENT: &str = r#"// Global event: Events.publish({Item}(...)), Events.subscribe<{Item}>(...), EventBuilder<{Item}>(...).
event {Item}
  message: string
"#;

const TEST: &str = r#"// Tests: @test (unit), @states (a statemanager, bloc_test) and @widget (a screen). @mock([X]) puts a
// mocktail mock of X in the injector; X.method() is the instance of the injector.
@mock([Hello{Name}UseCase])
{Name}Test
  setUp()
    when(() => Hello{Name}UseCase.exec()).thenReturn(Ok(["Hello"]))

  @test("the component says hello")
  hello()
    expect({Name}Component.hello(), isOkWith(["Hello"]))
"#;

const CONFIG: &str = r#"// Config of the component: its own connections (repositories use one with @repository(connection = "...")).
config {Name}Config
  connections: Connection[] = [
    Http(connectionName = "{name}", baseUrl = "https://api.example.com/"),
  ]
"#;

const SHARED: &str = r#"// Widget shared by every component.
stateless {Item}
  build(context: BuildContext) Widget
    Text("{Item}")
"#;

const STATEFUL: &str = r#"// A stateful widget shared by every component: it keeps its own state with setState.
stateful {Item}
  _count: int = 0

  _increment() void
    setState()
      _count += 1

  build(context: BuildContext) Widget
    TextButton(
      onPressed = _increment,
      child = Text("${_count}"),
    )
"#;

const GUARD: &str = r#"// Guard: an access check of its own (like CanActivate in NestJS). A screen uses it with
// @screen(route = "/x", acl = { guards: [{Item}Guard], forward: DeniedScreen }).
guard {Item}Guard
  (session: SessionManager)

  canActivate() boolean
    session.value<boolean>("{item}") != false
"#;

const GLOBAL: &str = r#"// Global component: any declaration can use it. A method with @on(Event) runs with every event of that
// type, wherever the user is. Publish one with: Events.publish({Item}(message = "..."))
component {Item}Component
  @on({Item})
  exec(event: {Item}) void
    debugPrint(event.message)
"#;

const PAGE: &str = r#"// A screen shared by the whole app (no component owns it).
@screen({screen})
stateless {Item}Screen
  build(context: BuildContext) Widget
    Scaffold(
      appBar = AppBar(title = Text("{Item}")),
      body = Center(child = Text("{Item}")),
    )
"#;

const INTEGRATION: &str = r#"// Integration test: the real app on a device or emulator (dragonfly test --integration -- -d <device>).
{Item}Test
  @widget("the app starts")
  starts(tester: WidgetTester)
    ~startDragonfly()
    ~tester.pumpWidget(App())
    ~tester.pumpAndSettle()
    expect(find.byType(App), findsOneWidget)
"#;

/// A new project: only .nuc files (main, config and a `home` component with its screen at `/`).
pub fn project(name: &str) -> Files {
    let n = pascal(name);
    let mut files = vec![
        (PathBuf::from("main.nuc"), fill(MAIN, &[("Name", &n)])),
        (PathBuf::from("config/project_config.nuc"), PROJECT_CONFIG.to_string()),
        (PathBuf::from(crate::packages::CONFIG), crate::packages::template(name)),
    ];
    files.extend(component("home"));
    files.extend(screen("home", "home", "/", "home_manager", "home_state"));
    files
}

/// A backend project (ADR-0093): its manifest and the app's connections; `nuclear build .` builds it with
/// the dialect (`fastapi`, `nestjs`). Components come from the platform's spec (`nuclear help backend`).
pub fn backend_project(name: &str, dialect: &str) -> Files {
    let config = format!(
        "// The platform this project is written for and the dialect it is built with (ADR-0093).\n\
         dragonfly(platform = \"backend\", dialect = \"{dialect}\")\n\
         app(name = \"{name}\", description = \"A Dragonfly backend.\", version = \"0.1.0\")\n\n\
         // The optional parts this project uses, only these go to the generated project (nuclear help backend):\n\
         // sqlite, postgres, mysql, mariadb, redis, kafka, events, ai, openai, ollama, chroma\n\
         plugins(sqlite = true)\n"
    );
    vec![
        (PathBuf::from(crate::packages::CONFIG), config),
        (PathBuf::from("config/project_config.nuc"), BACKEND_PROJECT_CONFIG.to_string()),
        (PathBuf::from("test/app_test.nuc"), BACKEND_TEST.to_string()),
    ]
}

/// A web project (ADR-0097): its manifest, the app entry (the router with every `@screen`) and the app's
/// connections; `nuclear build .` builds it with the `react` dialect. Screens come from the generators.
/// A web project of `dialect`; its `Http` connection points to `backend` (the url of the backend platform's default
/// dialect: where a new backend answers).
pub fn web_project(name: &str, dialect: &str, backend: &str) -> Files {
    let config = format!(
        "// The platform this project is written for and the dialect it is built with (ADR-0093, ADR-0097).\n\
         dragonfly(platform = \"web\", dialect = \"{dialect}\")\n\
         app(name = \"{name}\", description = \"A Dragonfly web app.\", version = \"0.1.0\")\n"
    );
    let base = format!("{}/", backend.trim_end_matches('/'));
    vec![
        (PathBuf::from(crate::packages::CONFIG), config),
        (PathBuf::from("main.nuc"), WEB_MAIN.to_string()),
        (PathBuf::from("config/project_config.nuc"), WEB_PROJECT_CONFIG.replace("{{backend}}", &base)),
    ]
}

const WEB_MAIN: &str = r#"// The app: the router with every @screen. index.ts (generated) starts Dragonfly and mounts App in #root.
stateless App
  build() Element
    createElement(RouterProvider, router = appRouter())
"#;

const WEB_PROJECT_CONFIG: &str = r#"// The connections and services of the whole app (nuclear help web).
appConfig ProjectConfig
  connections: Connection[] = [
    Http(baseUrl = "{{backend}}", timeout = 5000),
  ]
"#;

const BACKEND_TEST: &str = r#"// The first test (nuclear help backend, §2.13): every test runs with the connections open and the databases in
// memory; inject<X>() gives the app's instances, expect(condition) asserts.
tests App
  @test("the app starts with its connections")
  starts() void
    expect(true)
"#;

const BACKEND_PROJECT_CONFIG: &str = r#"// The connections of the whole app: databases, chat models, Kafka, Redis (nuclear help backend, §2.8).
// Each one needs its plugin in config.nuc.
appConfig ProjectConfig
  connections: Connection[] = [
    SQLite(file = env("DATABASE", "app.sqlite"), createTables = true),
  ]
"#;

pub fn component(c: &str) -> Files {
    let n = pascal(c);
    let base = PathBuf::from("components").join(c);
    vec![
        (base.join(format!("{c}_component.nuc")), fill(COMPONENT, &[("Name", &n), ("name", c)])),
        (
            base.join(format!("domain/usecases/hello_{c}_usecase.nuc")),
            fill(HELLO_USECASE, &[("Name", &n), ("name", c)]),
        ),
    ]
}

/// `make screen <c> <s>`: the screen, its state manager (`manager`, snake_case) and its state model
/// (`state`, snake_case).
pub fn screen(c: &str, s: &str, route: &str, manager: &str, state: &str) -> Files {
    let (n, sc, m, st) = (pascal(c), pascal(s), pascal(manager), pascal(state));
    let base = PathBuf::from("components").join(c).join("presentation").join(s);
    let vars = [
        ("Name", n.as_str()),
        ("Screen", sc.as_str()),
        ("Manager", m.as_str()),
        ("State", st.as_str()),
        ("route", route),
    ];
    vec![
        (base.join(format!("state_models/{state}.nuc")), fill(STATE, &vars)),
        (base.join(format!("managers/{manager}.nuc")), fill(MANAGER, &vars)),
        (base.join(format!("{s}_screen.nuc")), fill(SCREEN, &vars)),
    ]
}

/// `make config <c>`: the config of a component (its own connections).
pub fn config(c: &str) -> Files {
    let n = pascal(c);
    vec![(
        PathBuf::from("components").join(c).join(format!("config/{c}_config.nuc")),
        fill(CONFIG, &[("Name", &n), ("name", c)]),
    )]
}

/// `make usecase|repository|model|service|aggregate <component> <name>`.
pub fn in_component(what: &str, c: &str, item: &str) -> Option<Files> {
    let it = pascal(item);
    let vars = [("Item", it.as_str()), ("item", item), ("kind", what)];
    let base = PathBuf::from("components").join(c);
    let (path, text) = match what {
        "usecase" => (base.join(format!("domain/usecases/{item}_usecase.nuc")), USECASE),
        "repository" => (base.join(format!("data/{item}_repository.nuc")), REPOSITORY),
        "model" => (base.join(format!("domain/models/{item}.nuc")), MODEL),
        "service" => (base.join(format!("domain/services/{item}.nuc")), SERVICE),
        "aggregate" => (base.join(format!("domain/aggregates/{item}.nuc")), SERVICE),
        _ => return None,
    };
    let mut files = vec![(path, fill(text, &vars))];
    // a repository returns its model: create it when it does not exist
    if what == "repository" {
        files.push((base.join(format!("domain/models/{item}.nuc")), fill(MODEL, &vars)));
    }
    Some(files)
}

/// `make test <component>`: a test file for a component.
pub fn test(c: &str) -> Files {
    vec![(PathBuf::from(format!("test/{c}_test.nuc")), fill(TEST, &[("Name", &pascal(c))]))]
}

/// `make event <name>` and `make widget <name>`.
pub fn shared(what: &str, item: &str) -> Option<Files> {
    let it = pascal(item);
    let vars = [("Item", it.as_str())];
    match what {
        "event" => Some(vec![(PathBuf::from(format!("shared/events/{item}.nuc")), fill(EVENT, &vars))]),
        "widget" => Some(vec![(PathBuf::from(format!("shared/widgets/{item}.nuc")), fill(SHARED, &vars))]),
        "stateful" => Some(vec![(PathBuf::from(format!("shared/widgets/{item}.nuc")), fill(STATEFUL, &vars))]),
        _ => None,
    }
}

/// `make statemanager|statemodel <component> <screen> <name>`: a state manager (a Cubit) or a state model in
/// a screen's presentation folder (today only `make screen` created them). A standalone statemanager also
/// writes the state model it points at (`<name>` without `_manager`, plus `_state`).
pub fn presentation(what: &str, c: &str, s: &str, name: &str) -> Option<Files> {
    let base = PathBuf::from("components").join(c).join("presentation").join(s);
    match what {
        "statemodel" => {
            let st = pascal(name);
            Some(vec![(base.join(format!("state_models/{name}.nuc")), fill(STATE, &[("State", &st)]))])
        }
        "statemanager" => {
            let state =
                name.strip_suffix("_manager").map_or_else(|| format!("{name}_state"), |b| format!("{b}_state"));
            let (n, m, st) = (pascal(c), pascal(name), pascal(&state));
            let vars = [("Name", n.as_str()), ("Manager", m.as_str()), ("State", st.as_str())];
            Some(vec![
                (base.join(format!("state_models/{state}.nuc")), fill(STATE, &vars)),
                (base.join(format!("managers/{name}.nuc")), fill(MANAGER, &vars)),
            ])
        }
        _ => None,
    }
}

/// `make guard|global|page|integration <name>`: what lives outside the components. `screen` is the
/// `@screen(...)` arguments of a page. A repeated suffix is dropped (`premium_guard` → `PremiumGuard`).
pub fn app_wide(what: &str, name: &str, screen: &str) -> Option<Files> {
    let suffix = match what {
        "guard" => "_guard",
        "global" => "_component",
        "page" => "_screen",
        "integration" => "_test",
        _ => return None,
    };
    let item = name.strip_suffix(suffix).unwrap_or(name);
    let it = pascal(item);
    let vars = [("Item", it.as_str()), ("item", item), ("screen", screen)];
    let files = match what {
        "guard" => vec![(PathBuf::from(format!("shared/guards/{item}_guard.nuc")), fill(GUARD, &vars))],
        // the component listens to an event of its own name: create it when it does not exist
        "global" => vec![
            (PathBuf::from(format!("shared/components/{item}_component.nuc")), fill(GLOBAL, &vars)),
            (PathBuf::from(format!("shared/events/{item}.nuc")), fill(EVENT, &vars)),
        ],
        "page" => vec![(PathBuf::from(format!("shared/screens/{item}_screen.nuc")), fill(PAGE, &vars))],
        _ => vec![(PathBuf::from(format!("integration_test/{item}_test.nuc")), fill(INTEGRATION, &vars))],
    };
    Some(files)
}

/// Writes the files under `root`; an existing file is kept (generators never overwrite code).
pub fn write(root: &Path, files: &Files) -> std::io::Result<Vec<String>> {
    let mut report = Vec::new();
    for (rel, text) in files {
        let p = root.join(rel);
        if p.exists() {
            report.push(format!(
                "  {}    {}",
                crate::landing::dim("kept"),
                crate::landing::dim(&rel.display().to_string())
            ));
            continue;
        }
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&p, text)?;
        report.push(format!("  {} {}", crate::landing::ok("created"), rel.display()));
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(pascal("user_profile"), "UserProfile");
        assert_eq!(snake("UserProfile"), "user_profile");
        assert_eq!(snake("user-profile"), "user_profile");
    }

    #[test]
    fn every_skeleton_parses() {
        let mut files = project("demo_app");
        files.extend(backend_project("demo_api", "fastapi"));
        files.extend(web_project("demo_web", "react", "http://localhost:8000"));
        files.extend(in_component("usecase", "home", "find_user").unwrap());
        files.extend(in_component("repository", "home", "user").unwrap());
        files.extend(in_component("service", "home", "user_mapper").unwrap());
        files.extend(in_component("aggregate", "home", "cart").unwrap());
        files.extend(shared("event", "force_update").unwrap());
        files.extend(shared("widget", "offline_banner").unwrap());
        files.extend(shared("stateful", "counter_box").unwrap());
        files.extend(config("home"));
        files.extend(screen("home", "profile", "/profile", "account_manager", "account_view_state"));
        files.extend(presentation("statemanager", "home", "detail", "detail_manager").unwrap());
        files.extend(presentation("statemodel", "home", "detail", "extra_state").unwrap());
        files.extend(test("home"));
        files.extend(app_wide("guard", "premium_guard", "").unwrap());
        files.extend(app_wide("global", "force_logout", "").unwrap());
        files.extend(app_wide("page", "denied", r#"route = "/denied""#).unwrap());
        files.extend(app_wide("page", "not_found", "notFound = true").unwrap());
        files.extend(app_wide("integration", "app", "").unwrap());
        assert!(files.iter().any(|(p, _)| p.ends_with("shared/guards/premium_guard.nuc")));
        for (path, text) in files.iter().filter(|(p, _)| p.extension().is_some_and(|e| e == "nuc")) {
            let parsed = nuclear_syntax_check(text);
            assert!(parsed.is_empty(), "{}: {parsed:?}", path.display());
        }
    }

    fn nuclear_syntax_check(text: &str) -> Vec<String> {
        nuclear_transpile::check(text, None).iter().map(|d| d.message.clone()).collect()
    }
}
