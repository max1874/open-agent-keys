# open-agent-keys

`key` gives AI coding agents (Claude Code, Codex, Cursor, …) the API keys they need without the values ever entering the conversation.

```console
$ key list openai
OPENAI_SCRIPTS	[Personal/OpenAI]	Scripts and evals; created at platform.openai.com, project "scripts"
$ key run OPENAI_API_KEY=OPENAI_SCRIPTS -- python3 eval.py
```

The agent sees names and notes. The value goes from your password manager straight into the environment of the one command that needs it, so it is not in the transcript, the model provider's logs, your shell history, or a `.env` file the agent wrote somewhere.

Stores: **1Password** (service accounts) and **Bitwarden Secrets Manager** (machine accounts). One Python file, standard library only, Python 3.9+.

## Why

An agent that needs a key usually gets it one of three ways, and each leaks:

- **Pasted into the chat.** The value now lives in the transcript, on disk and at the model provider.
- **Read from `.env` files.** Copies spread across repositories, and an agent looking for a key learns to grep your disk for one.
- **Your password manager's CLI, signed in as you.** The agent can read every password you own.

`key` closes all three. It signs in as a separate machine identity that can see only the vaults or projects you give it, it never prints a value, and when it fails it tells the agent to stop and report instead of searching elsewhere.

## Setup

### 1. Install

```sh
mkdir -p ~/.local/bin
curl -fsSL https://raw.githubusercontent.com/max1874/open-agent-keys/main/key -o ~/.local/bin/key
chmod +x ~/.local/bin/key
```

or clone this repository and symlink `key` onto your `PATH`. You also need your store's CLI: `op` for 1Password, `bws` for Bitwarden.

### 2a. 1Password

1. Create a vault for agent keys, e.g. `Agents`. Keep personal passwords out of it.
2. Create a [service account](https://developer.1password.com/docs/service-accounts/get-started/) with read and write access to that vault only.
3. Save its token and find the vault ID:

   ```sh
   mkdir -p ~/.config/key && chmod 700 ~/.config/key
   pbpaste > ~/.config/key/token && chmod 600 ~/.config/key/token   # the service account token
   OP_SERVICE_ACCOUNT_TOKEN=$(cat ~/.config/key/token) op vault list
   ```

4. Write `~/.config/key/config.ini`:

   ```ini
   [key]
   store = 1password

   [scope Personal]
   id = <vault id>
   ```

Keys are API Credential items: the value in `credential`, the note in the item's notes. Login items work too (their `password` is the value). Service accounts have a request quota (`op service-account ratelimit`), so `key` caches names and notes and fetches a note only when its item changed.

### 2b. Bitwarden Secrets Manager

1. In the Bitwarden web app, turn on Secrets Manager for your organization (the free plan is enough for one person) and create a project, e.g. `agents`.
2. Create a machine account, give it read and write access to that project only, and create an access token for it.
3. Install [`bws`](https://bitwarden.com/help/secrets-manager-cli/), save the token and find the project ID:

   ```sh
   mkdir -p ~/.config/key && chmod 700 ~/.config/key
   pbpaste > ~/.config/key/token && chmod 600 ~/.config/key/token   # the access token
   BWS_ACCESS_TOKEN=$(cat ~/.config/key/token) bws project list
   ```

4. Write `~/.config/key/config.ini`:

   ```ini
   [key]
   store = bitwarden

   [scope Personal]
   id = <project id>
   ```

Differences from 1Password: secrets have no tags, `key rm` deletes for good (there is no archive), and `bws` takes a new value as a command-line argument, so `key add` exposes it to `ps` for as long as `bws` runs. For EU or self-hosted servers, set `BWS_SERVER_URL` in your environment.

Bitwarden's regular password manager CLI (`bw`) is deliberately not supported: it unlocks with your master password and then sees your whole vault, which is exactly what `key` exists to prevent.

### 3. Tell your agents

Copy [`AGENTS.snippet.md`](AGENTS.snippet.md) into your global agent instructions (`~/.claude/CLAUDE.md`, `~/.codex/AGENTS.md`, …). The tool keeps values out of the transcript; the instructions keep the agent from going around it.

## Scopes

A scope is one vault (1Password) or project (Bitwarden). List several to keep keys apart, for instance your own and your employer's:

```ini
[key]
store = 1password
token_file = ~/.config/key/token   ; the default

[scope Personal]
id = <vault id>

[scope Work]
id = <vault id>
prefix = WORK_
```

A key's name is its scope's prefix plus its title in upper case with spaces as underscores: "Gemini Prod" in `Work` is `WORK_GEMINI_PROD`. The name alone tells an agent which side a key belongs to, so a personal script cannot pick up a company key by accident. In 1Password, tags nest under the scope (`Work/Gemini`), so the sidebar never mixes the two.

`key add` puts a key in the first scope unless you pass `--scope`.

## Commands

| Command | What it does |
|---|---|
| `key list [WORD]` | Names, tags and notes, filtered by WORD. Never values. |
| `key run NAME[=SECRET]... -- CMD` | Runs CMD with only those secrets added to its environment. `NAME=SECRET` exposes secret SECRET as env var NAME. |
| `key env FILE NAME[=SECRET]...` | Writes or replaces those lines in a dotenv file, mode 600. For tools that only read files. |
| `key add "Title" --note TEXT [--scope S] [--tag T]...` | Creates a key from one line on stdin: `pbpaste \| key add "OpenAI Scripts" --note "…" --tag OpenAI`. |
| `key note NAME TEXT` | Replaces a key's note. |
| `key rm NAME` | Removes a key: 1Password moves it to the Archive, Bitwarden deletes it. |

Agent sandboxes: `key` needs network access and read access to `~/.config/key`. It works when that directory is read-only; it just lists the store on every lookup instead of using its cache. Set `KEY_CONFIG_DIR` to keep the config elsewhere.

## What it does not protect against

- A command that prints its own environment, or a script that logs the key. `key run` puts the value in the child's environment; what the child does with it is up to the child.
- An agent that ignores its instructions and runs `key env` into a file it then reads. The instructions and the transcript make that visible, not impossible.
- Anyone who can read `~/.config/key/token`: that token is the machine identity. Keep the file at mode 600, and revoke the service account or machine account if the machine is compromised.

What it does guarantee: the agent's credential sees only the scopes you granted, never your personal vault, and no value passes through `key`'s own output.

## License

MIT
