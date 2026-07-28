//! DevTools KRunner Runner
//!
//! A Plasma 6 DBus runner (DBus2 protocol). KRunner calls our service on the
//! session bus (`org.kde.devtools` at object path `/runner`) and we answer
//! `org.kde.krunner1` method calls.
//!
//! MVP scope: typing `date` / `time` (or a prefix like `da` / `tim`) shows
//! several time formats; pressing Enter copies the selected value and shows a
//! desktop notification.
//!
//! On-wire match shape: `(Id, Text, IconName, CategoryRelevance, Relevance, Properties)`
//! which serializes to the DBus signature `a(sssida{sv})`.
//!
//! NOTE: the 4th field (`i`) is `categoryRelevance`, NOT "type" — the
//! installed `kf6_org.kde.krunner1.xml` comment that calls it "Type" is stale.
//! See `dbusutils_p.h` in the KRunner framework: `RemoteMatch::categoryRelevance`
//! defaults to `Lowest` (= 0). Sending 0 puts every match in the lowest sort
//! bucket, so results sink to the bottom. We send `Highest` (= 100).

#![allow(non_snake_case)]

use std::collections::HashMap;
use std::process::Command;
use std::time::Duration;

use chrono::{DateTime, Local, Utc};
use zbus::blocking::ConnectionBuilder;
use zbus::zvariant::{OwnedValue, Value};

/// The DBus bus name we register.
const SERVICE_NAME: &str = "org.kde.devtools";
/// Object path KRunner will call (must match the .desktop `X-Plasma-DBusRunner-Path`).
const OBJECT_PATH: &str = "/runner";
/// KRunner category used to group our results.
const CATEGORY: &str = "DevTools";
/// `KRunner::QueryMatch::CategoryRelevance::Highest` (see dbusutils_p.h).
/// Highest so our results sort near the top instead of in the default Lowest bucket.
const CATEGORY_RELEVANCE: i32 = 100;

/// Keyword stems that activate this runner.
const KEYWORDS: [&str; 2] = ["date", "time"];

/// Static definition of every result row: `(id suffix, title, icon name)`.
/// Values are computed at query time so they are always current.
const ITEMS: &[(&str, &str, &str)] = &[
    ("local", "当前时间", "clock"),
    ("unix", "Unix 时间戳", "preferences-system-time"),
    ("unixms", "Unix 时间戳 (ms)", "preferences-system-time"),
    ("rfc3339", "RFC3339", "text-x-generic"),
    ("iso8601", "ISO8601", "text-x-generic"),
    ("utc", "UTC 时间", "clock"),
];

/// `(Id, Text, IconName, CategoryRelevance, Relevance, Properties)` -> DBus `a(sssida{sv})`.
type KMatch = (String, String, String, i32, f64, HashMap<String, OwnedValue>);

struct DevTools;

/// Wrap a string as an `OwnedValue` (DBus variant `v`) for the `a{sv}` dict.
/// `OwnedValue` only has `From` for primitives, so we go through `Value`.
fn str_value(s: impl Into<String>) -> OwnedValue {
    OwnedValue::try_from(Value::from(s.into()))
        .expect("OwnedValue::try_from(Value) is infallible")
}

/// The copyable value for a row id suffix, computed from a fixed point in time.
fn value_of(suffix: &str, now: &DateTime<Local>, utc: &DateTime<Utc>) -> String {
    match suffix {
        "local" => now.format("%Y-%m-%d %H:%M:%S").to_string(),
        "unix" => now.timestamp().to_string(),
        "unixms" => now.timestamp_millis().to_string(),
        "rfc3339" => now.format("%Y-%m-%dT%H:%M:%S%:z").to_string(),
        "iso8601" => utc.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        "utc" => utc.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        _ => String::new(),
    }
}

/// Build the full result list for a `date`/`time` query.
fn build_matches() -> Vec<KMatch> {
    let now = Local::now();
    let utc = Utc::now();
    ITEMS
        .iter()
        .enumerate()
        .map(|(i, (suffix, title, icon))| {
            let value = value_of(suffix, &now, &utc);
            let mut props = HashMap::new();
            props.insert("subtext".to_string(), str_value(value));
            props.insert("category".to_string(), str_value(CATEGORY));
            (
                format!("date:{suffix}"),
                (*title).to_string(),
                (*icon).to_string(),
                CATEGORY_RELEVANCE, // categoryRelevance: Highest -> sorts near the top
                1.0 - 0.03 * i as f64, // relevance: orders items within the category
                props,
            )
        })
        .collect()
}

/// Does this query look like a date/time request?
fn matches(query: &str) -> bool {
    let q = query.trim().to_lowercase();
    if q.len() < 2 {
        return false;
    }
    // A keyword stem is a prefix of the query, or the query is a prefix of a stem.
    KEYWORDS.iter().any(|k| q.starts_with(k) || k.starts_with(&q))
}

/// Resolve the copy value for a match id we previously produced.
fn value_for_id(id: &str) -> Option<String> {
    let suffix = id.strip_prefix("date:")?;
    let now = Local::now();
    let utc = Utc::now();
    let value = value_of(suffix, &now, &utc);
    (!value.is_empty()).then_some(value)
}

/// Copy text to the Wayland clipboard. `wl-copy` stays alive as the clipboard
/// owner, so we spawn-and-detach rather than blocking on it.
fn copy_to_clipboard(text: &str) {
    match Command::new("wl-copy").arg(text).spawn() {
        Ok(_) => {}
        Err(e) => eprintln!("devtools-runner: wl-copy failed: {e}"),
    }
}

/// Show a desktop notification.
fn notify(summary: &str, body: &str) {
    if let Err(e) = Command::new("notify-send")
        .args(["--app-name", "DevTools", "--icon", "edit-copy", summary, body])
        .spawn()
    {
        eprintln!("devtools-runner: notify-send failed: {e}");
    }
}

/// `org.kde.krunner1` DBus interface.
#[zbus::interface(name = "org.kde.krunner1")]
impl DevTools {
    /// Return matching results for a query.
    fn Match(&self, query: &str) -> Vec<KMatch> {
        let items = if matches(query) {
            build_matches()
        } else {
            Vec::new()
        };
        eprintln!("devtools-runner: Match {query:?} -> {} item(s)", items.len());
        items
    }

    /// Supported actions. None in the MVP (default action = copy).
    fn Actions(&self) -> Vec<(String, String, String)> {
        Vec::new()
    }

    /// Execute the default action for a match (copy + notify).
    fn Run(&self, match_id: &str, _action_id: &str) -> zbus::fdo::Result<()> {
        match value_for_id(match_id) {
            Some(value) => {
                eprintln!("devtools-runner: copy '{match_id}' -> {value}");
                copy_to_clipboard(&value);
                notify("Copied", &value);
                Ok(())
            }
            None => Err(zbus::fdo::Error::Failed(format!(
                "unknown match id: {match_id}"
            ))),
        }
    }

    /// Runtime runner config. Empty = let the runner decide relevance itself.
    fn Config(&self) -> HashMap<String, OwnedValue> {
        HashMap::new()
    }

    /// Called when a match session is over. Nothing to clean up yet.
    fn Teardown(&self) {}
}

fn main() {
    eprintln!("devtools-runner: starting {SERVICE_NAME} at {OBJECT_PATH}");
    let _connection = match ConnectionBuilder::session()
        .and_then(|b| b.name(SERVICE_NAME))
        .and_then(|b| b.serve_at(OBJECT_PATH, DevTools))
        .and_then(|b| b.build())
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("devtools-runner: failed to start: {e}");
            std::process::exit(1);
        }
    };
    eprintln!("devtools-runner: ready");

    // The connection's internal executor dispatches incoming DBus messages to
    // the object server on a background thread. Keep the process alive.
    loop {
        std::thread::sleep(Duration::from_secs(3600));
    }
}
