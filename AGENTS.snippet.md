## API keys

- Reach keys only through `open-agent-keys` (`open-agent-keys --help`). Run `open-agent-keys list` before asking for a key or creating one.
- Use `open-agent-keys run NAME[=SECRET]... -- <cmd>` or `open-agent-keys env <file> NAME...` so a value never appears in output or the transcript; never print, echo, or paste one, and never run a command that would.
- If `open-agent-keys` fails, report the failure. Do not search files, shell history, app configs, or browser profiles for keys.
- Never use the password manager's own app, browser extension, or CLI signed in as the user: those see the user's personal vault. Never read `open-agent-keys`'s token file or call `op`/`bws` with it yourself.
- A key whose name starts with a scope prefix (such as `WORK_`) serves that scope's work only, and a key without one never serves it.
- Create a key with `open-agent-keys add`, with a note naming the provider, the purpose, and the console it was created in. When a key is replaced, revoke the old one at the provider and `open-agent-keys rm` it.
