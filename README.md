# open-agent-keys

`open-agent-keys` gives AI coding agents (Claude Code, Codex, Cursor, …) the API keys they need without the values ever entering the conversation.

```console
$ open-agent-keys list openai
OPENAI_SCRIPTS	[Personal/OpenAI]	Scripts and evals; created at platform.openai.com, project "scripts"
$ open-agent-keys run OPENAI_API_KEY=OPENAI_SCRIPTS -- python3 eval.py
```

The agent sees names and notes. The value goes from your password manager straight into the environment of the one command that needs it, so it is not in the transcript, the model provider's logs, your shell history, or a `.env` file the agent wrote somewhere.

Stores: **1Password** (service accounts) and **Bitwarden Secrets Manager** (machine accounts). One Python file, standard library only, Python 3.9+.

## Why

An agent that needs a key usually gets it one of three ways, and each leaks:

- **Pasted into the chat.** The value now lives in the transcript, on disk and at the model provider.
- **Read from `.env` files.** Copies spread across repositories, and an agent looking for a key learns to grep your disk for one.
- **Your password manager's CLI, signed in as you.** The agent can read every password you own.

`open-agent-keys` closes all three. It signs in as a separate machine identity that can see only the vaults or projects you give it, it never prints a value, and when it fails it tells the agent to stop and report instead of searching elsewhere.

## Setup

### 1. Install

```sh
mkdir -p ~/.local/bin
curl -fsSL https://raw.githubusercontent.com/max1874/open-agent-keys/main/open-agent-keys -o ~/.local/bin/open-agent-keys
chmod +x ~/.local/bin/open-agent-keys
```

or clone this repository and symlink `open-agent-keys` onto your `PATH`. You also need your store's CLI: `op` for 1Password, `bws` for Bitwarden.

### 2a. 1Password

1. Create a vault for agent keys, e.g. `Agents`. Keep personal passwords out of it.
2. Create a [service account](https://developer.1password.com/docs/service-accounts/get-started/) with read and write access to that vault only.
3. Save its token and find the vault ID:

   ```sh
   mkdir -p ~/.config/open-agent-keys && chmod 700 ~/.config/open-agent-keys
   pbpaste > ~/.config/open-agent-keys/token && chmod 600 ~/.config/open-agent-keys/token   # the service account token
   OP_SERVICE_ACCOUNT_TOKEN=$(cat ~/.config/open-agent-keys/token) op vault list
   ```

4. Write `~/.config/open-agent-keys/config.ini`:

   ```ini
   [store]
   type = 1password

   [scope Personal]
   id = <vault id>
   ```

Keys are API Credential items: the value in `credential`, the note in the item's notes. Login items work too (their `password` is the value). Service accounts have a request quota (`op service-account ratelimit`), so `open-agent-keys` caches names and notes and fetches a note only when its item changed.

### 2b. Bitwarden Secrets Manager

1. In the Bitwarden web app, turn on Secrets Manager for your organization (the free plan is enough for one person) and create a project, e.g. `agents`.
2. Create a machine account, give it read and write access to that project only, and create an access token for it.
3. Install [`bws`](https://bitwarden.com/help/secrets-manager-cli/), save the token and find the project ID:

   ```sh
   mkdir -p ~/.config/open-agent-keys && chmod 700 ~/.config/open-agent-keys
   pbpaste > ~/.config/open-agent-keys/token && chmod 600 ~/.config/open-agent-keys/token   # the access token
   BWS_ACCESS_TOKEN=$(cat ~/.config/open-agent-keys/token) bws project list
   ```

4. Write `~/.config/open-agent-keys/config.ini`:

   ```ini
   [store]
   type = bitwarden

   [scope Personal]
   id = <project id>
   ```

Differences from 1Password: secrets have no tags, `open-agent-keys rm` deletes for good (there is no archive), and `bws` takes a new value as a command-line argument, so `open-agent-keys add` exposes it to `ps` for as long as `bws` runs. For EU or self-hosted servers, set `BWS_SERVER_URL` in your environment.

Bitwarden's regular password manager CLI (`bw`) is deliberately not supported: it unlocks with your master password and then sees your whole vault, which is exactly what `open-agent-keys` exists to prevent.

### 3. Tell your agents

Copy [`AGENTS.snippet.md`](AGENTS.snippet.md) into your global agent instructions (`~/.claude/CLAUDE.md`, `~/.codex/AGENTS.md`, …). The tool keeps values out of the transcript; the instructions keep the agent from going around it.

## Scopes

A scope is one vault (1Password) or project (Bitwarden). List several to keep keys apart, for instance your own and your employer's:

```ini
[store]
type = 1password
; the default token file
token_file = ~/.config/open-agent-keys/token

[scope Personal]
id = <vault id>

[scope Work]
id = <vault id>
prefix = WORK_
```

A key's name is its scope's prefix plus its title in upper case with spaces as underscores: "Gemini Prod" in `Work` is `WORK_GEMINI_PROD`. The name alone tells an agent which side a key belongs to, so a personal script cannot pick up a company key by accident. In 1Password, tags nest under the scope (`Work/Gemini`), so the sidebar never mixes the two.

`open-agent-keys add` puts a key in the first scope unless you pass `--scope`.

## Commands

| Command | What it does |
|---|---|
| `open-agent-keys list [WORD]` | Names, tags and notes, filtered by WORD. Never values. |
| `open-agent-keys run NAME[=SECRET]... -- CMD` | Runs CMD with only those secrets added to its environment. `NAME=SECRET` exposes secret SECRET as env var NAME. |
| `open-agent-keys env FILE NAME[=SECRET]...` | Writes or replaces those lines in a dotenv file, mode 600. For tools that only read files. |
| `open-agent-keys add "Title" --note TEXT [--scope S] [--tag T]...` | Creates a key from one line on stdin: `pbpaste \| open-agent-keys add "OpenAI Scripts" --note "…" --tag OpenAI`. |
| `open-agent-keys note NAME TEXT` | Replaces a key's note. |
| `open-agent-keys rm NAME` | Removes a key: 1Password moves it to the Archive, Bitwarden deletes it. |

Agent sandboxes: `open-agent-keys` needs network access and read access to `~/.config/open-agent-keys`. It works when that directory is read-only; it just lists the store on every lookup instead of using its cache. Set `OPEN_AGENT_KEYS_CONFIG_DIR` to keep the config elsewhere.

## What it does not protect against

- A command that prints its own environment, or a script that logs the key. `open-agent-keys run` puts the value in the child's environment; what the child does with it is up to the child.
- An agent that ignores its instructions and runs `open-agent-keys env` into a file it then reads. The instructions and the transcript make that visible, not impossible.
- Anyone who can read `~/.config/open-agent-keys/token`: that token is the machine identity. Keep the file at mode 600, and revoke the service account or machine account if the machine is compromised.

What it does guarantee: the agent's credential sees only the scopes you granted, never your personal vault, and no value passes through `open-agent-keys`'s own output.

## License

MIT
