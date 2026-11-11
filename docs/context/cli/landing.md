---
layer: L3
component: cli
read-when: you change the landing page, the animation, the header, the help or the colors of the CLI output
---

# Landing, header, help and colors (`landing.rs`)

- `dragonfly` without arguments: the animation ONLY the first time (marker `$XDG_STATE_HOME/dragonfly/intro_seen`,
  removed by `./install.sh --uninstall`), then the page alone. `dragonfly credits` (alias `author`): always the
  full animation and the page with credits (Julian Molina, github.com/nodelorien, github.com/nuclearlang).
- ~11 s animation as a CONCEPT, not realistic: radioactive mushroom in ASCII, a wave of characters that fills the
  screen and dissolves into `█▓▒░`, grass grows back and a dragonfly lands on a flower; then the page with the
  NuclearLang and Dragonfly logos. After the wave, "WELCOME" (5-row letters) appears in negative, turns green
  with "to NuclearLang · Dragonfly" and dissolves. In the grass scene the motto is written (`motto`): Spanish
  if `LC_ALL`/`LC_MESSAGES`/`LANG` starts with `es` (localization data, allowed by ADR-0075/0082), else English.
- Only characters over the terminal background: no background colors (a test checks it), the theme's text color
  in normal/bold/dim and few accents; fades are dissolves. The dragonfly is metallic purple (`metal`) with
  radioactive green eyes; the NuclearLang symbol is drawn `ASPECT` (1.35) times wider to look round.
- The page stays alive until Enter (a thread reads stdin; Enter during the animation jumps to the page, except
  in `credits`; EOF only closes a complete page); a shine crosses the dragonfly every `SHINE_EVERY` (5 s);
  without a terminal stdin it ends by itself.
- `header()`: before any command (also `-h`/`help`; not on the bare page nor `credits`), a 6-row header with
  both small logos (`MINI`, `sign` radius 6), on stderr and only in a terminal.
- Output colors: `ok` green = done, `bad` red = error, `warn` yellow = warning/note, `cmd` blue = commands,
  `name` purple = project/step names, `dim` = paths and details; `diagnostics()` colors transpiler errors; only if
  stdout and stderr are terminals and no `NO_COLOR`. In `watch`, `dragonfly: transpiling|ready|errors` has no
  color (the VS Code task looks for it).
- `usage()`: help with a width-1 symbol per section and command (⚙ build, ✿ new, ⚗ test...; only symbols in
  common monospace fonts, checked with `fc-list :charset=`). Without a terminal, with `NO_COLOR` or below 80×23,
  only the page.
- Review the animation without a terminal: `script -qc "stty cols 100 rows 30; dragonfly" /dev/null` captures
  frames (split by `ESC[?2026h`); a PIL script turns them into images. The user has not yet checked the new
  animation in a real terminal (`STATUS.md`).
