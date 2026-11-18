---
name: open-agent-keys
description: "Give a command the API key, token or other secret it needs without the value entering the conversation: find keys by name and note, run a command with them as env vars, write them to a mode-600 dotenv file, add or retire a key. Use whenever a script, CLI, API call or test login needs a credential, before asking the user for one, and before creating one."
---

# open-agent-keys

`open-agent-keys` reads keys from a password-manager scope the user granted to a
machine identity (a 1Password service account or a Bitwarden Secrets Manager
machine account). You see names and notes; a value goes straight into the
environment of the one command that needs it.

**For commands and their syntax, run `open-agent-keys --help`.** The installed
binary prints it, so it always matches the version you have. This file holds only
what stays the same across versions.

## Rules

- Reach keys only through `open-agent-keys`. Never print, echo or paste a value,
  and never run a command that would (`env`, `printenv`, `echo $KEY`, `cat` of a
  file `env` wrote, a debug flag that logs headers).
- If a command fails, report the error to the user and stop. Do not search files,
  shell history, app configs or browser profiles for keys, and do not retry in
  another form.
- Never read its token file or call `op` or `bws` with it yourself. Never use the
  password manager's own app, browser extension, or CLI signed in as the user:
  those see the user's personal vault.
- A name that starts with a scope prefix (such as `WORK_`) serves that scope's work
  only; a name without one never serves it.

## Working with keys

- Run `list` before asking the user for a key or creating one, and pick by the
  note, which says what the key is for.
- Prefer `run`: only the listed secrets reach the child. Don't run the child with
  flags that print its environment or request headers.
- Use `env` only for a tool that reads a file. Keep that file out of version
  control, and source it inside the command that needs it rather than reading it.
- `add` reads the value from stdin (`pbpaste | open-agent-keys add …`), so it never
  appears in an argument; without `--scope` it goes to the first scope. Write the
  note for the next agent: the provider, the purpose, and the console it was
  created in.
- When a key is replaced, revoke the old one at the provider, then `rm` it.
