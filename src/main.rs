//! open-agent-keys: give AI agents API keys without putting the values in the transcript.

use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{self, Command, Output, Stdio};

const HELP: &str = r#"open-agent-keys: give AI agents API keys without putting the values in the transcript.

Keys live in a password manager the agent can only partly see: a 1Password service account
or a Bitwarden Secrets Manager machine account, limited to the vaults or projects listed as
scopes in ~/.config/open-agent-keys/config.ini. Agents see names and notes; a value goes
straight into the environment of the command that needs it.

  open-agent-keys list [WORD]                  names, tags and notes; never values
  open-agent-keys run NAME[=SECRET]... -- CMD  run CMD with only those secrets as env vars
  open-agent-keys env FILE NAME[=SECRET]...    write/replace those lines in a dotenv file (mode 600)
  open-agent-keys add "Title" --note TEXT [--scope SCOPE] [--tag TAG]...
                                               create from stdin (`pbpaste | open-agent-keys add ...`)
  open-agent-keys note NAME TEXT               replace a key's note
  open-agent-keys rm NAME                      remove a key (1Password archives it, Bitwarden deletes it)

A key's NAME is its scope's prefix plus its title in upper case, spaces as underscores:
"Gemini Prod" in a scope with prefix WORK_ is WORK_GEMINI_PROD.
NAME=SECRET exposes secret SECRET under env name NAME, e.g.
  open-agent-keys run OPENAI_API_KEY=OPENAI_SCRIPTS -- python3 script.py"#;

fn die(msg: impl std::fmt::Display) -> ! {
    eprintln!("open-agent-keys: {msg}");
    process::exit(2)
}

fn expand(path: &str) -> PathBuf {
    match path.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => {
            let home = env::var("HOME").unwrap_or_else(|_| die("HOME is not set"));
            PathBuf::from(home + rest)
        }
        _ => PathBuf::from(path),
    }
}

fn text(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_owned()
}

/// One key as the store lists it. `note` is None until fetched (1Password lists without notes).
struct Entry {
    id: String,
    title: String,
    tags: Vec<String>,
    updated_at: String,
    note: Option<String>,
    scope: String,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    OnePassword,
    Bitwarden,
}

/// A password-manager CLI signed in with a token that sees only the configured scopes.
struct Store {
    kind: Kind,
    token: String,
}

impl Store {
    fn cli(&self) -> &'static str {
        match self.kind {
            Kind::OnePassword => "op",
            Kind::Bitwarden => "bws",
        }
    }

    fn call(&self, args: &[&str], stdin: Option<&str>) -> Output {
        let mut cmd = Command::new(self.cli());
        match self.kind {
            // No cache daemon: its socket lives outside the paths sandboxed agents may write.
            Kind::OnePassword => cmd.env("OP_SERVICE_ACCOUNT_TOKEN", &self.token).env("OP_CACHE", "false"),
            Kind::Bitwarden => cmd.env("BWS_ACCESS_TOKEN", &self.token).args(["--output", "json"]),
        };
        cmd.args(args)
            .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd.spawn().unwrap_or_else(|e| match e.kind() {
            ErrorKind::NotFound => die(format!("`{}` is not installed; see the README", self.cli())),
            _ => die(format!("cannot run {}: {e}", self.cli())),
        });
        if let Some(input) = stdin {
            // A CLI that exits early closes the pipe; its own stderr, reported below, says why.
            let _ = child.stdin.take().expect("stdin is piped").write_all(input.as_bytes());
        }
        child.wait_with_output().unwrap_or_else(|e| die(format!("{} did not finish: {e}", self.cli())))
    }

    fn fail(&self, out: &Output) -> ! {
        let err: String = String::from_utf8_lossy(&out.stderr).trim().chars().take(300).collect();
        die(format!("{} failed (tell the user; do not look for keys elsewhere): {err}", self.cli()))
    }

    fn check(&self, args: &[&str], stdin: Option<&str>) -> Vec<u8> {
        let out = self.call(args, stdin);
        if !out.status.success() {
            self.fail(&out)
        }
        out.stdout
    }

    fn query(&self, args: &[&str], stdin: Option<&str>) -> Value {
        let out = self.check(args, stdin);
        if out.trim_ascii().is_empty() {
            return Value::Null;
        }
        // Never echo the output: it may carry values.
        serde_json::from_slice(&out).unwrap_or_else(|_| die(format!("{} printed something other than JSON", self.cli())))
    }

    fn items(&self, scope: &Scope) -> Vec<Entry> {
        let (list, title, updated_at) = match self.kind {
            Kind::OnePassword => (self.query(&["item", "list", "--vault", &scope.id, "--format", "json"], None), "title", "updated_at"),
            Kind::Bitwarden => (self.query(&["secret", "list", &scope.id], None), "key", "revisionDate"),
        };
        // The Bitwarden listing carries values too; only these fields leave this method.
        let entry = |i: &Value| Entry {
            id: text(&i["id"]),
            title: text(&i[title]),
            tags: i["tags"].as_array().map(|t| t.iter().map(text).collect()).unwrap_or_default(),
            updated_at: text(&i[updated_at]),
            note: (self.kind == Kind::Bitwarden).then(|| text(&i["note"])),
            scope: scope.name.clone(),
        };
        list.as_array().map(Vec::as_slice).unwrap_or_default().iter().map(entry).collect()
    }

    fn note(&self, scope: &str, item: &str) -> String {
        let args = ["item", "get", item, "--vault", scope, "--fields", "label=notesPlain", "--format", "json"];
        text(&self.query(&args, None)["value"])
    }

    fn read(&self, scope: &str, item: &str) -> Option<String> {
        match self.kind {
            Kind::OnePassword => {
                // API Credential items keep the value in `credential`, Login items in `password`.
                for field in ["credential", "password"] {
                    let out = self.call(&["read", "--no-newline", &format!("op://{scope}/{item}/{field}")], None);
                    if out.status.success() {
                        return Some(String::from_utf8(out.stdout).unwrap_or_else(|_| die("the value is not UTF-8")));
                    }
                    let err = String::from_utf8_lossy(&out.stderr);
                    if err.contains("isn't an item") {
                        return None;
                    }
                    if !err.contains("does not have a field") {
                        self.fail(&out)
                    }
                }
                die("the item has neither a `credential` nor a `password` field")
            }
            Kind::Bitwarden => {
                let out = self.call(&["secret", "get", item], None);
                if out.status.success() {
                    let s: Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|_| die("bws printed something other than JSON"));
                    // A secret moved to another project after the cache was written belongs to that scope now.
                    return (s["projectId"] == scope).then(|| text(&s["value"]));
                }
                if String::from_utf8_lossy(&out.stderr).contains("404") {
                    return None;
                }
                self.fail(&out)
            }
        }
    }

    fn create(&self, scope: &str, title: &str, value: &str, note: &str, tags: &[String]) {
        match self.kind {
            Kind::OnePassword => {
                // The value goes in on stdin as a template, never as an argument other processes can see.
                let item = json!({"title": title, "category": "API_CREDENTIAL", "tags": tags, "fields": [
                    {"id": "credential", "type": "CONCEALED", "label": "credential", "value": value},
                    {"id": "notesPlain", "type": "STRING", "purpose": "NOTES", "label": "notesPlain", "value": note},
                ]});
                self.query(&["item", "create", "--vault", scope, "--format", "json", "-"], Some(&item.to_string()));
            }
            // bws takes the value only as an argument, visible to `ps` for as long as bws runs.
            Kind::Bitwarden => {
                self.query(&["secret", "create", &format!("--note={note}"), "--", title, value, scope], None);
            }
        }
    }

    fn set_note(&self, scope: &str, item: &str, text: &str) {
        match self.kind {
            // op echoes the edited item, values included; query() parses it and prints nothing.
            Kind::OnePassword => self.query(&["item", "edit", item, "--vault", scope, "--format", "json", &format!("notesPlain={text}")], None),
            Kind::Bitwarden => self.query(&["secret", "edit", item, &format!("--note={text}")], None),
        };
    }

    fn remove(&self, scope: &str, item: &str) {
        match self.kind {
            Kind::OnePassword => self.check(&["item", "delete", item, "--vault", scope, "--archive"], None),
            // Plain text ("1 secret deleted successfully.") even with --output json.
            Kind::Bitwarden => self.check(&["secret", "delete", item], None),
        };
    }
}

struct Scope {
    name: String,
    id: String,
    prefix: String,
}

/// configparser's dialect as far as config.ini needs it: sections, `key = value` or `key: value`,
/// comments on their own line or after whitespace. Anything else is an error, never a guess.
fn parse_ini(path: &Path, text: &str) -> Vec<(String, BTreeMap<String, String>)> {
    let mut sections: Vec<(String, BTreeMap<String, String>)> = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let bad = |what: &str| -> ! { die(format!("{}:{}: {what}", path.display(), n + 1)) };
        let end = line
            .char_indices()
            .find(|&(i, c)| (c == ';' || c == '#') && (line[..i].trim().is_empty() || line[..i].ends_with(char::is_whitespace)))
            .map_or(line.len(), |(i, _)| i);
        let line = line[..end].trim();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            if sections.iter().any(|(s, _)| s == name.trim()) {
                bad("section appears twice")
            }
            sections.push((name.trim().to_owned(), BTreeMap::new()));
            continue;
        }
        let Some((_, section)) = sections.last_mut() else { bad("setting before any [section]") };
        let Some((key, value)) = line.split_once(['=', ':']) else { bad("expected `key = value`") };
        if section.insert(key.trim().to_lowercase(), value.trim().to_owned()).is_some() {
            bad("key appears twice in its section")
        }
    }
    sections
}

struct Keys {
    store: Store,
    scopes: Vec<Scope>,
    dir: PathBuf,
}

impl Keys {
    fn load() -> Keys {
        let dir = match env::var_os("OPEN_AGENT_KEYS_CONFIG_DIR") {
            Some(d) => d.to_str().map_or_else(|| PathBuf::from(&d), expand),
            None => expand("~/.config/open-agent-keys"),
        };
        let path = dir.join("config.ini");
        let ini = fs::read_to_string(&path).unwrap_or_else(|_| die(format!("no config at {}; see the README", path.display())));
        let sections = parse_ini(&path, &ini);
        let none = BTreeMap::new();
        let store = sections.iter().find(|(name, _)| name == "store").map_or(&none, |(_, s)| s);
        let kind = match store.get("type").map(String::as_str) {
            Some("1password") => Kind::OnePassword,
            Some("bitwarden") => Kind::Bitwarden,
            _ => die(format!("{}: set `type` under [store] to one of: 1password, bitwarden", path.display())),
        };
        let scopes: Vec<Scope> = sections
            .iter()
            .filter_map(|(name, s)| {
                Some(Scope {
                    name: name.strip_prefix("scope ")?.to_owned(),
                    id: s.get("id").cloned().unwrap_or_default(),
                    prefix: s.get("prefix").map(|p| p.to_uppercase()).unwrap_or_default(),
                })
            })
            .collect();
        if scopes.is_empty() || scopes.iter().any(|s| s.id.is_empty()) {
            die(format!("{}: add at least one [scope NAME] section with an id", path.display()))
        }
        let token_file = store.get("token_file").map_or_else(|| dir.join("token"), |t| expand(t));
        let token = fs::read_to_string(&token_file)
            .unwrap_or_else(|e| die(format!("cannot read {}: {e}; tell the user, do not look for keys elsewhere", token_file.display())))
            .trim()
            .to_owned();
        if token.is_empty() {
            die(format!("{} is empty; tell the user", token_file.display()))
        }
        Keys { store: Store { kind, token }, scopes, dir }
    }

    fn scope(&self, name: &str) -> Option<&Scope> {
        self.scopes.iter().find(|s| s.name == name)
    }

    fn name(scope: &Scope, title: &str) -> String {
        let mut name = String::new();
        for c in title.chars() {
            if c.is_ascii_alphanumeric() {
                name.push(c.to_ascii_uppercase());
            } else if !name.ends_with('_') {
                name.push('_');
            }
        }
        format!("{}{}", scope.prefix, name.trim_matches('_'))
    }

    fn by_name(&self, entries: Vec<Entry>) -> Result<BTreeMap<String, Entry>, String> {
        let mut named: BTreeMap<String, Entry> = BTreeMap::new();
        for e in entries {
            let Some(scope) = self.scope(&e.scope) else { continue };
            let name = Self::name(scope, &e.title);
            if let Some(other) = named.get(&name) {
                // Picking either could hand one scope's key to another scope's task.
                return Err(format!(
                    "\"{}\" ({}) and \"{}\" ({}) are both {name}; rename one",
                    other.title, other.scope, e.title, e.scope
                ));
            }
            named.insert(name, e);
        }
        Ok(named)
    }

    /// Names, tags and notes, never values. It lets `run` skip listing the store, and 1Password skip
    /// re-fetching notes that did not change, which would count against a service account's quota.
    fn cache_file(&self) -> PathBuf {
        self.dir.join("cache.json")
    }

    fn load_cache(&self) -> Map<String, Value> {
        let raw = fs::read(self.cache_file()).unwrap_or_default();
        match serde_json::from_slice(&raw) {
            Ok(Value::Object(map)) => map,
            _ => Map::new(),
        }
    }

    fn cached(&self) -> BTreeMap<String, Entry> {
        let entries: Option<Vec<Entry>> = self
            .load_cache()
            .into_iter()
            .map(|(id, e)| {
                Some(Entry {
                    title: e["title"].as_str()?.to_owned(),
                    scope: e["scope"].as_str()?.to_owned(),
                    tags: Vec::new(),
                    updated_at: String::new(),
                    note: None,
                    id,
                })
            })
            .collect();
        // Stale or malformed: the caller lists the store.
        entries.and_then(|e| self.by_name(e).ok()).unwrap_or_default()
    }

    fn index(&self) -> BTreeMap<String, Entry> {
        let old = self.load_cache();
        let mut fresh = Vec::new();
        for scope in &self.scopes {
            for mut e in self.store.items(scope) {
                if e.note.is_none() {
                    let prev = old.get(&e.id).filter(|p| p["updated_at"] == e.updated_at.as_str());
                    e.note = Some(match prev.and_then(|p| p["note"].as_str()) {
                        Some(note) => note.to_owned(),
                        None => self.store.note(&scope.id, &e.id),
                    });
                }
                fresh.push(e);
            }
        }
        self.save_cache(&fresh);
        self.by_name(fresh).unwrap_or_else(|e| die(e))
    }

    fn save_cache(&self, entries: &[Entry]) {
        let map: Map<String, Value> = entries
            .iter()
            .map(|e| {
                let v = json!({"title": e.title, "tags": e.tags, "updated_at": e.updated_at, "note": e.note, "scope": e.scope});
                (e.id.clone(), v)
            })
            .collect();
        // A config dir agents may not write (some sandboxes) just means the next run lists again.
        let _ = fs::DirBuilder::new().recursive(true).mode(0o700).create(&self.dir).and_then(|_| {
            let mut f = fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(self.cache_file())?;
            f.write_all(Value::Object(map).to_string().as_bytes())
        });
    }

    fn entry(&self, name: &str) -> Entry {
        self.index().remove(name).unwrap_or_else(|| die(format!("no secret named {name}; see `open-agent-keys list`")))
    }

    fn read(&self, name: &str) -> String {
        for lookup in [Self::cached, Self::index] {
            if let Some(e) = lookup(self).get(name) {
                let scope = self.scope(&e.scope).expect("by_name keeps known scopes");
                if let Some(value) = self.store.read(&scope.id, &e.id) {
                    return value;
                }
            }
        }
        die(format!("no secret named {name}; see `open-agent-keys list`"))
    }

    /// `NAME[=SECRET]` specs to (env name, value), in order, a repeated name keeping its first place.
    fn pick(&self, specs: &[String]) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = Vec::new();
        for spec in specs {
            let (name, secret) = spec.split_once('=').unwrap_or((spec, ""));
            let value = self.read(if secret.is_empty() { name } else { secret });
            match out.iter_mut().find(|(n, _)| n == name) {
                Some(slot) => slot.1 = value,
                None => out.push((name.to_owned(), value)),
            }
        }
        out
    }
}

fn cmd_list(keys: &Keys, args: &[String]) {
    let word = args.first().map(|w| w.to_lowercase()).unwrap_or_default();
    for (name, e) in keys.index() {
        let tags = e.tags.join(",");
        let note = e.note.unwrap_or_default();
        if format!("{name} {} {tags} {note}", e.title).to_lowercase().contains(&word) {
            println!("{name}\t[{tags}]\t{note}");
        }
    }
}

/// Takes raw arguments: the command after `--` may name files that are not UTF-8.
fn cmd_run(keys: &Keys, args: &[OsString]) {
    fn usage() -> ! {
        die("usage: open-agent-keys run NAME[=SECRET]... -- CMD")
    }
    let Some(i) = args.iter().position(|a| a == "--") else { usage() };
    let (specs, cmd) = (utf8(&args[..i]), &args[i + 1..]);
    if specs.is_empty() || cmd.is_empty() {
        usage()
    }
    let err = Command::new(&cmd[0]).args(&cmd[1..]).envs(keys.pick(&specs)).exec();
    die(format!("cannot run {}: {err}", cmd[0].to_string_lossy()))
}

/// `NAME=` at the start of a dotenv line, with any leading `export`: (everything before NAME, NAME).
fn assignment(line: &str) -> Option<(&str, &str)> {
    let indented = line.trim_start();
    let exported = indented.strip_prefix("export").filter(|r| r.starts_with(char::is_whitespace)).map(str::trim_start);
    exported.into_iter().chain([indented]).find_map(|rest| {
        let end = rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(rest.len());
        let name = &rest[..end];
        let named = name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_');
        (named && rest[end..].trim_start().starts_with('=')).then(|| (&line[..line.len() - rest.len()], name))
    })
}

fn dotenv_value(name: &str, value: &str) -> String {
    if value.chars().all(|c| c.is_ascii_alphanumeric() || "_.,:@/+=-".contains(c)) {
        return value.to_owned();
    }
    if value.contains(['\'', '\n', '\r']) {
        die(format!("{name} holds a quote or line break that dotenv files cannot carry reliably; use `open-agent-keys run`"))
    }
    // Single quotes keep $ and spaces literal for shells, Docker Compose and python-dotenv alike.
    format!("'{value}'")
}

fn cmd_env(keys: &Keys, args: &[String]) {
    if args.len() < 2 {
        die("usage: open-agent-keys env FILE NAME[=SECRET]...")
    }
    let path = Path::new(&args[0]);
    let mut values = keys.pick(&args[1..]);
    for (k, v) in &mut values {
        *v = dotenv_value(k, v);
    }
    let existing = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == ErrorKind::NotFound => String::new(),
        Err(e) => die(format!("cannot read {}: {e}", path.display())),
    };
    let mut done = Vec::new();
    let mut lines: Vec<String> = existing
        .lines()
        .map(|line| {
            let Some((prefix, name)) = assignment(line) else { return line.to_owned() };
            match values.iter().find(|(k, _)| k == name) {
                Some((k, v)) => {
                    done.push(k.as_str());
                    format!("{prefix}{k}={v}")
                }
                None => line.to_owned(),
            }
        })
        .collect();
    lines.extend(values.iter().filter(|(k, _)| !done.contains(&k.as_str())).map(|(k, v)| format!("{k}={v}")));
    let written = fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path).and_then(|mut f| {
        // An existing file keeps its mode through O_TRUNC; tighten it before writing.
        f.set_permissions(fs::Permissions::from_mode(0o600))?;
        f.write_all((lines.join("\n") + "\n").as_bytes())
    });
    written.unwrap_or_else(|e| die(format!("cannot write {}: {e}", path.display())));
    let names: Vec<&str> = values.iter().map(|(k, _)| k.as_str()).collect();
    println!("wrote {} to {}", names.join(", "), path.display());
}

fn cmd_add(keys: &Keys, args: &[String]) {
    let usage = "usage: open-agent-keys add \"Title\" --note TEXT [--scope SCOPE] [--tag TAG]...";
    let (mut title, mut note, mut scope, mut tags) = (None, None, None, Vec::new());
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((flag, value)) if flag.starts_with("--") => (flag, Some(value.to_owned())),
            _ => (arg.as_str(), None),
        };
        let mut value = || {
            let next = || it.next().filter(|v| !v.starts_with("--")).cloned();
            inline.clone().or_else(next).unwrap_or_else(|| die(format!("{flag} needs a value")))
        };
        match flag {
            "--note" => note = Some(value()),
            "--scope" => scope = Some(value()),
            "--tag" => tags.push(value()),
            _ if arg.starts_with('-') || title.is_some() => die(usage),
            _ => title = Some(arg.clone()),
        }
    }
    let (Some(title), Some(note)) = (title, note) else { die(usage) };
    let scope = match scope {
        Some(s) => keys.scope(&s).unwrap_or_else(|| {
            let names: Vec<&str> = keys.scopes.iter().map(|s| s.name.as_str()).collect();
            die(format!("--scope must be one of: {}", names.join(", ")))
        }),
        None => &keys.scopes[0],
    };
    let well_formed = title.starts_with(|c: char| c.is_ascii_alphabetic())
        && !title.ends_with(' ')
        && title.chars().all(|c| c.is_ascii_alphanumeric() || c == ' ');
    if !well_formed {
        die("title must be words of letters and digits, e.g. \"OpenAI Scripts\"")
    }
    if !tags.is_empty() && keys.store.kind == Kind::Bitwarden {
        die("bws has no tags; leave out --tag")
    }
    let name = Keys::name(scope, &title);
    if keys.index().contains_key(&name) {
        die(format!("{name} already exists; pick another title"))
    }
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap_or_else(|e| die(format!("cannot read stdin: {e}")));
    let value = input.trim();
    if value.is_empty() || value.contains(['\n', '\r']) {
        die("expected exactly one non-empty line on stdin")
    }
    // 1Password nests tags at "/", so each scope's tags group under it and never mix with another's.
    let nested = format!("{}/", scope.name);
    let tags: Vec<String> = tags.into_iter().map(|t| if t.starts_with(&nested) { t } else { nested.clone() + &t }).collect();
    keys.store.create(&scope.id, &title, value, &note, &tags);
    println!("added {title} as {name}");
}

fn cmd_note(keys: &Keys, args: &[String]) {
    let [name, text] = args else { die("usage: open-agent-keys note NAME TEXT") };
    let e = keys.entry(name);
    let scope = keys.scope(&e.scope).expect("by_name keeps known scopes");
    keys.store.set_note(&scope.id, &e.id, text);
    keys.index();
    println!("updated note of {name}");
}

fn cmd_rm(keys: &Keys, args: &[String]) {
    let [name] = args else { die("usage: open-agent-keys rm NAME") };
    let e = keys.entry(name);
    let scope = keys.scope(&e.scope).expect("by_name keeps known scopes");
    keys.store.remove(&scope.id, &e.id);
    keys.index();
    println!("removed {name}");
}

fn utf8(args: &[OsString]) -> Vec<String> {
    let text = |a: &OsString| a.to_str().map(str::to_owned).unwrap_or_else(|| die(format!("argument {} is not UTF-8", a.to_string_lossy())));
    args.iter().map(text).collect()
}

fn main() {
    let args: Vec<OsString> = env::args_os().skip(1).collect();
    let cmd = args.first().map(|c| c.to_string_lossy().into_owned()).unwrap_or_default();
    let rest = &args[args.len().min(1)..];
    let run: fn(&Keys, &[String]) = match cmd.as_str() {
        "" | "-h" | "--help" | "help" => return println!("{HELP}"),
        "run" => return cmd_run(&Keys::load(), rest),
        "list" => cmd_list,
        "env" => cmd_env,
        "add" => cmd_add,
        "note" => cmd_note,
        "rm" => cmd_rm,
        _ => die(format!("unknown command {cmd}; see `open-agent-keys --help`")),
    };
    run(&Keys::load(), &utf8(rest));
}
