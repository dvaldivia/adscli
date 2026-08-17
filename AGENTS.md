# adscli

Agent-first Google Ads CLI (API v25). Discover the interface from the binary — do not search the web.

```
adscli --help
adscli schema --json
adscli <noun> --help
adscli <noun> <verb> --help
adscli version --json
```

Rules:
- Every command supports `--help` and `--json`.
- JSON on stdout; errors on stderr (JSON when `--json`).
- Subcommands never prompt and never block.
- `--limit` defaults to 50. Pass `--limit 0` only if you must.
- Mutations accept `--dry-run`. Live mutations require `--yes`.
- Creates default to `PAUSED`.
- Bare `adscli` is a TUI and fails without a TTY. Use a subcommand.
- Do not run `adscli login` from a script. Set `ADSCLI_REFRESH_TOKEN` (and the Desktop client id/secret). Check `adscli auth status --json`.

Exit codes: `0` ok, `1` error, `2` usage, `3` not found, `4` auth, `5` conflict.
