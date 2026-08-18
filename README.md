# adscli

Agent-first CLI and k9s-style TUI for the [Google Ads API v25](https://developers.google.com/google-ads/api/docs/release-notes).

Browse campaigns, asset groups, and assets; pull performance; create or edit them. Every command is self-describing via `--help`. Agents should start with `adscli schema --json`.

```
┌ adscli  1234567890  CAMPAIGNS > Brand  LAST_30_DAYS  12 rows ─────────────┐
│ ID        NAME          STATUS    TYPE               IMPR   CLICKS  COST  │
│ 111       Brand         ENABLED   SEARCH             9021   340     88.12 │
│ 222       PMax          PAUSED    PERFORMANCE_MAX    1402    41     19.00 │
│                                                                           │
│ ?:help /:filter ⏎:open esc:back e:enable p:pause d:describe r:reload q:quit│
└───────────────────────────────────────────────────────────────────────────┘
```

## Install

Pre-built binaries are published for linux × macOS × x86_64 + arm64.

**Homebrew (macOS and Linux)** — same tap as the rest of the `dvaldivia` tools:

```sh
brew install dvaldivia/tap/adscli
```

**Pre-built tarball** from the [latest release](https://github.com/dvaldivia/adscli/releases/latest):

```sh
# Pick your tarball — linux/x86_64, linux/arm64, darwin/x86_64, darwin/arm64
VERSION=0.0.2
OS=linux ARCH=x86_64
curl -sSL -o adscli.tar.gz \
  "https://github.com/dvaldivia/adscli/releases/download/v${VERSION}/adscli_${OS}_${ARCH}.tar.gz"
tar -xzf adscli.tar.gz
install -m 0755 adscli /usr/local/bin/adscli
```

`checksums.txt` in each release covers all four tarballs.

**Cargo install (from source):**

```sh
cargo install --git https://github.com/dvaldivia/adscli --tag v0.0.2 adscli
```

**Build from source:**

```sh
git clone https://github.com/dvaldivia/adscli
cd adscli
cargo build --release -p adscli
install -m 0755 target/release/adscli ~/.local/bin/adscli
```

## Agent workflow

```sh
adscli version --json
adscli schema --json
adscli login
adscli auth status --json
adscli customers list --json
adscli campaigns list --json --limit 20
adscli asset-groups list --campaign 123 --json
adscli assets list --asset-group 20 --json
adscli performance campaigns --during LAST_7_DAYS --json
adscli campaigns pause 123 --dry-run --json
adscli campaigns pause 123 --yes --json
```

`--json` is global. Errors are JSON on stderr when `--json` is set. Nothing prompts. The default command (no subcommand) is a TUI and refuses to start without a TTY.

## Auth

Two separate credentials:

| Credential | Default | What it is for |
|---|---|---|
| OAuth Desktop client (`client_id` + `client_secret`) | **built into every binary** | Proves *who* you are. `adscli login` exchanges a browser consent for a refresh token. |
| Developer token | **not shipped** | Permits the *app* to call the API. Sent as `developer-token` on every request after login. |

A developer token cannot sign you in. Login cannot call the API without a developer token.

Same facts live in `adscli login --help` and `adscli --help`.

### Default OAuth client

Every adscli build (Homebrew, release tarball, `cargo install`, local `cargo build`) compiles in this Desktop client:

```sh
# compiled defaults — you do not need to export these
ADSCLI_CLIENT_ID=REDACTED
ADSCLI_CLIENT_SECRET=REDACTED
```

`adscli login` uses them unless something higher in the stack overrides. Typical first-time setup is only:

```sh
export ADSCLI_DEVELOPER_TOKEN=...   # still required for API calls
adscli login
adscli auth status --json           # has_oauth_client + oauth_from_bundle
```

Resolution, highest wins:

1. `--client-id` / `--client-secret`
2. `ADSCLI_CLIENT_ID` / `ADSCLI_CLIENT_SECRET`
3. `client_id` / `client_secret` in the config file or `credentials.json`
4. the compiled defaults above

When (4) wins, `adscli auth status --json` and `adscli config show --json` report `has_oauth_client: true` and `oauth_from_bundle: true`. Those commands never print the secret. `has_bundled_oauth` is true in every current build (the defaults exist even if you overrode them for this process).

Official release CI can still replace the compiled client (or inject a developer token) with `ADSCLI_BUNDLED_CLIENT_ID` / `ADSCLI_BUNDLED_CLIENT_SECRET` / `ADSCLI_BUNDLED_DEVELOPER_TOKEN` at compile time.

Until Google [verifies](https://support.google.com/cloud/answer/9110914) the adscli consent screen, only **test users** listed on that screen can finish login. Everyone else hits the unverified-app wall (`Advanced → Go to adscli`). Branding (name, logo, [homepage](https://adscli.dev), [privacy policy](https://adscli.dev/privacy.html)) is what users see on the Allow page. Users still sign in as themselves; they do not create a Cloud project.

### Why the secret is in the binary

Google treats Desktop / installed apps as public clients: they [cannot keep secrets](https://developers.google.com/identity/protocols/oauth2/native-app). Anyone with the binary (or this README) can read the client id and secret. That is expected.

A stolen client secret is not a password for anyone’s Google Ads account. It only lets another program say “I am the adscli OAuth client.” Google still requires a human to click Allow. Existing refresh tokens stay on each user’s machine (keychain or `credentials.json`), not in the binary.

What a leak *does* enable:

- a lookalike CLI that shows the adscli consent screen (users should install from Homebrew or the GitHub releases)
- abuse of *this* Cloud project’s OAuth client (Google can disable it; rotate the secret in Cloud Console and cut a new release)

PKCE (S256) protects the loopback `?code=` exchange. It does not hide the secret.

The developer token is the sharper credential: it is the app’s API permit and quota. It is **not** compiled in. A leak of that token, plus any valid user refresh token, counts against adscli’s Ads quota and can get the token suspended. Official releases may bake one in via `ADSCLI_BUNDLED_DEVELOPER_TOKEN`; treat that as an ops incident if it leaks, not a user-data breach.

### Developer token

1. Sign in to a Google Ads **manager** account (MCC).
2. **Admin → API Center** (or search “API Center”).
3. Apply for / copy the [developer token](https://developers.google.com/google-ads/api/docs/get-started/dev-token) (22-character string).
4. A **test-access** token only works against test accounts. Production accounts need **Basic** or higher.

`adscli login` does **not** send this token. Every later command (`customers`, `campaigns`, …) does.

```sh
export ADSCLI_DEVELOPER_TOKEN=...
```

or in `~/.config/adscli/config.yaml` / `.adscli.yaml` (Python `google-ads.yaml` field names work too):

```yaml
developer_token: "..."
# client_id / client_secret — omit to use the built-in adscli client
# customer_id: "123-456-7890"
# login_customer_id: "098-765-4321"   # MCC, if you use one
```

### Bring your own client

Only needed to use a **different** Google Cloud project.

1. Create or pick a [Google Cloud project](https://console.cloud.google.com/).
2. Enable the [Google Ads API](https://console.cloud.google.com/apis/library/googleads.googleapis.com).
3. **APIs & Services → OAuth consent screen** — External is fine; add yourself as a test user while unverified; add scope `https://www.googleapis.com/auth/adwords`.
4. **Credentials → Create credentials → OAuth client ID → Desktop app** (not Web). Download the JSON. Desktop clients already allow `http://127.0.0.1`; do not register a port.
5. Override the built-ins:

```sh
export ADSCLI_CLIENT_ID=....apps.googleusercontent.com
export ADSCLI_CLIENT_SECRET=...
export ADSCLI_DEVELOPER_TOKEN=...
adscli login
adscli auth status --json    # oauth_from_bundle: false
```

### Sign in

```sh
adscli login                    # opens the browser (PKCE + loopback)
# adscli login --device         # no local HTTP server
# adscli login --print-url      # print the URL, do not open a browser
adscli auth status --json       # has_refresh_token + has_developer_token to call the API
adscli config show --json       # same facts, plus oauth_from_bundle
```

The refresh token is stored in the OS keychain, or in `~/.config/adscli/credentials.json` (mode `0600`) if the keychain is unavailable. Later commands refresh the access token silently.

As of August 2026, Google may require a **passkey** when issuing a **new** Ads refresh token. Existing tokens keep working.

### Pick an account

```sh
adscli customers list --json
export ADSCLI_CUSTOMER_ID=1234567890          # dashes ok
export ADSCLI_LOGIN_CUSTOMER_ID=0987654321    # required when the user sits under an MCC
adscli campaigns list --json --limit 5
```

### Agents

Do not run `adscli login` from a script. After a human has logged in once, set `ADSCLI_REFRESH_TOKEN` and `ADSCLI_DEVELOPER_TOKEN` (client id/secret only if you are not using the built-in client) or point `--config` at the YAML above. Check `adscli auth status --json`.

## Commands

| Command | Purpose |
|---|---|
| `adscli` | k9s-style TUI (TTY required) |
| `version` | binary + API version |
| `schema` | full command tree as JSON |
| `login` / `logout` | browser SSO (Desktop OAuth + PKCE) |
| `auth login\|status\|logout` | same login, plus status |
| `config show\|path` | redacted settings |
| `customers list\|get` | accessible accounts |
| `campaigns list\|get\|create\|update\|enable\|pause\|remove` | campaigns |
| `asset-groups …` | Performance Max asset groups |
| `assets list\|get\|create\|update\|link\|unlink` | assets |
| `performance campaigns\|asset-groups\|assets` | metrics |
| `gaql --query 'SELECT …'` | raw GAQL |

Creates default to `PAUSED`. Live mutations require `--yes`. `--dry-run` prints the mutate payload and does not call the API.

## TUI keys

| Key | Action |
|---|---|
| `j` / `k` / `↑` / `↓` | move |
| `Enter` / `l` | drill down (campaigns → asset groups → assets) |
| `Esc` / `h` / `q` | back (quit at the top level) |
| `/` | filter |
| `e` / `p` | enable / pause |
| `d` | describe |
| `r` | reload |
| `?` | help |

## Exit codes

`0` ok · `1` error · `2` usage · `3` not found · `4` auth · `5` conflict
