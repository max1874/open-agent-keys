---
name: open-agent-keys
description: "Give a command the API key, token or other secret it needs without the value entering the conversation: find keys by name and note, run a command with them as env vars, write them to a mode-600 dotenv file, add or retire a key. Use whenever a script, CLI, API call or test login needs a credential, before asking the user for one, and before creating one."
---

# open-agent-keys

`open-agent-keys` reads keys from a password-manager scope the user granted to a
machine identity (a 1Password service account or a Bitwarden Secrets Manager
machine account). You see names and notes; a value goes straight into the
environment of the one command that needs it.

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

## Find a key

```bash
open-agent-keys list            # every name, tag and note
open-agent-keys list gemini     # filtered by a word
```

Pick by the note, which says what the key is for. Run `list` before asking the user
for a key or creating one.

## Use a key

```bash
open-agent-keys run OPENAI_SCRIPTS -- python3 eval.py
open-agent-keys run OPENAI_API_KEY=OPENAI_SCRIPTS -- python3 eval.py   # rename
open-agent-keys run A B -- cmd                                          # several
```

`NAME=SECRET` exposes secret `SECRET` under env name `NAME`. Only the listed
secrets reach the child, and the child gets them, so do not run it with flags that
print its environment or request headers.

For a tool that only reads a file:

```bash
open-agent-keys env .env.local OPENAI_API_KEY=OPENAI_SCRIPTS
```

This writes or replaces those lines in a mode-600 dotenv file. Keep that file out
of version control, and source it inside the command that needs it rather than
reading it.

## Add, annotate, remove

```bash
pbpaste | open-agent-keys add "OpenAI Scripts" --tag OpenAI \
  --note "Scripts and evals; platform.openai.com, project scripts" [--scope Work]
open-agent-keys note OPENAI_SCRIPTS "new note"
open-agent-keys rm OPENAI_SCRIPTS
```

- `add` reads one line from stdin, so the value never appears in an argument. Write
  the note for the next agent: the provider, the purpose, and the console it was
  created in. Without `--scope` it goes to the first scope.
- When a key is replaced, revoke the old one at the provider, then `rm` it.
  1Password archives it (restorable); Bitwarden deletes it permanently.
- A key's name is its scope's prefix plus its title in upper case, spaces as
  underscores. Two titles that map to the same name stop every command until the
  user renames one.
