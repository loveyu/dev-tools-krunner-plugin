# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

A **Plasma 6 KRunner DBus runner** (Rust + `zbus`). KRunner calls a session-bus
service we expose; we answer `date`/`time` queries with several time formats and
copy the chosen value to the clipboard on Enter. MVP scope is date/time inline;
the design doc (in the original request / `README.md`) describes a later
external-plugin-manager architecture.

Target: KDE Plasma 6 + Wayland (developed on 6.3.6 / Frameworks 6.13).

## Commands

```bash
cargo build --release              # build (cargo installed at ~/.cargo via rustup)
./install.sh                       # build + deploy + restart KRunner (re-run after any change)
./target/release/devtools-runner   # run in foreground (stderr logs) for debugging
```

No tests yet. The verification loop is DBus-based (see Debugging).

## Debugging / verification

```bash
# Introspect the interface/signature we expose:
qdbus6 org.kde.devtools /runner
dbus-send --session --print-reply --dest=org.kde.devtools /runner \
    org.freedesktop.DBus.Introspectable.Introspect

# Call Match directly (bypasses KRunner). Look at the int32 (categoryRelevance) and double (relevance):
dbus-send --session --print-reply --dest=org.kde.devtools /runner \
    org.kde.krunner1.Match string:"date"

# Open KRunner filtered to this runner:
qdbus6 org.kde.krunner /App org.kde.krunner.App.querySingleRunner org.kde.devtools date

# The service logs Match/Run calls to stderr. When DBus-activated, stderr is
# discarded — so to see krunner actually calling us, run the binary in
# foreground yourself (it owns the bus name) and watch the terminal.
```

Install locations (per-user, no root): binary → `~/.local/bin/devtools-runner`;
KRunner metadata → `~/.local/share/krunner/dbusplugins/org.kde.devtools.desktop`;
D-Bus activation → `~/.local/share/dbus-1/services/org.kde.devtools.service`.
DBus auto-activates the service on first query, so nothing must be started manually.

## The KRunner DBus2 protocol contract (critical, non-obvious)

- **Interface is `org.kde.krunner1`** (the "DBus2" protocol), object path `/runner`,
  bus name `org.kde.devtools`. This is *not* `org.kde.krunner.App` (that's krunner's
  own client interface at `/App`). The authoritative on-system reference is
  `/usr/share/dbus-1/interfaces/kf6_org.kde.krunner1.xml`.
- **`Match(query: s) → a(sssida{sv})`** — each match is the struct
  `(Id, Text, IconName, CategoryRelevance:i32, Relevance:f64, Properties:a{sv})`.
- Also: `Run(matchId:s, actionId:s)`, `Actions() → a(sss)`, `Config() → a{sv}`,
  `Teardown()`.
- **The 4th field (`i`) is `categoryRelevance`, NOT "type".** The installed
  `kf6_org.kde.krunner1.xml` comments call it "Type" — that comment is STALE.
  Ground truth is `RemoteMatch` in the KRunner framework's `src/dbusutils_p.h`:
  `int categoryRelevance = ...::Lowest;` (defaults to 0). Sending 0 sinks every
  result to the bottom. We send `100` (`Highest`).
- The `.desktop` keys that matter (grepped from `libKF6Runner.so`): `X-Plasma-API=DBus2`,
  `X-Plasma-DBusRunner-Service=<busname>`, `X-Plasma-DBusRunner-Path=/runner`.
  Runner discovery dir is `~/.local/share/krunner/dbusplugins/`.
- Properties dict keys the consumer reads: `subtext`, `category`, `urls`,
  `multiline`, `icon-data`, `actions`. (`categoryRelevance` is a struct field, not a dict key.)

## KRunner result ordering (why results rank where they do)

From `resultsmodel.cpp` `SortProxyModel::lessThan`:
- **Category-level** (groups like "Applications", "DevTools"): sort by
  `(FavoriteIndex, CategoryRelevance)`.
- **Match-level** (within a category): sort by `relevance` only.

Consequences:
- `QueryMatch::setCategoryRelevance` is clamped to `[0, 100]`; `setRelevance` is
  **unclamped above 0**. So a huge relevance only reorders within our category —
  it cannot lift our category above another.
- `categoryRelevance = 100` is the ceiling. Core runners (Applications, System
  Settings) also use `Highest` for their strong matches (e.g. the "Date & Time"
  settings module); at a tie KRunner orders categories by load/insertion order
  (core first), so a DBus runner **cannot programmatically force itself above**
  other `Highest` core categories.
- `FavoriteIndexRole` is `/// @internal` and comes only from user-pinned match IDs
  (the star icon in KRunner) — not settable by a runner. Pinning is the only way
  to guarantee absolute-top placement.

## zbus gotchas (Rust)

- Cargo feature is **`blocking`**, not `blocking-api` (`zbus = { version = "4",
  features = ["async-io", "blocking"] }`).
- `OwnedValue` has no `From<String>`/`From<&str>` — wrap via
  `OwnedValue::try_from(Value::from(s))`. See `str_value` in `main.rs`.
- Interface methods are written PascalCase (`Match`, `Run`, `Actions`, `Config`,
  `Teardown`) so the D-Bus member names match exactly; `#![allow(non_snake_case)]`
  silences the lint.
- Blocking server = `ConnectionBuilder` `.name().serve_at().build()` then a
  sleep loop. The internal `async-io` executor dispatches incoming messages on a
  background thread — **no manual `receive_message` loop is needed**.

## Adding a new result type

Add a row to `ITEMS` (`(id_suffix, title, icon)`) and a branch to `value_of`.
Match IDs are `date:<suffix>` and are recomputed in `value_for_id` at Run time
(time is always current). No other plumbing changes.
