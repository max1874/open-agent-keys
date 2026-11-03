## API keys

- Reach keys only through `key` (`key --help`). Run `key list` before asking for a key or creating one.
- Use `key run NAME[=SECRET]... -- <cmd>` or `key env <file> NAME...` so a value never appears in output or the transcript; never print, echo, or paste one, and never run a command that would.
- If `key` fails, report the failure. Do not search files, shell history, app configs, or browser profiles for keys.
- Never use the password manager's own app, browser extension, or CLI signed in as the user: those see the user's personal vault.
- A key whose name starts with a scope prefix (such as `WORK_`) serves that scope's work only, and a key without one never serves it.
- Create a key with `key add`, with a note naming the provider, the purpose, and the console it was created in. When a key is replaced, revoke the old one at the provider and `key rm` it.
