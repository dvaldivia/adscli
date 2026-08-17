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
VERSION=0.0.1
OS=linux ARCH=x86_64
curl -sSL -o adscli.tar.gz \
  "https://github.com/dvaldivia/adscli/releases/download/v${VERSION}/adscli_${OS}_${ARCH}.tar.gz"
tar -xzf adscli.tar.gz
install -m 0755 adscli /usr/local/bin/adscli
```

`checksums.txt` in each release covers all four tarballs.

**Cargo install (from source):**

```sh
cargo install --git https://github.com/dvaldivia/adscli --tag v0.0.1 adscli
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

You need **two credentials from two consoles**. A developer token alone cannot sign you in.

| Credential | Where it comes from | What it is for |
|---|---|---|
| OAuth Desktop client (`client_id` + `client_secret`) | [Google Cloud Console](https://console.cloud.google.com/) | Proves *who* you are. `adscli login` uses this to get a refresh token. |
| Developer token | Google Ads **Admin → API Center** | Permits the *app* to call the API. Sent as `developer-token` on every request after login. |

adscli does **not** ship a shared OAuth client. The `adwords` scope is sensitive, so every user or company creates their own. `adscli login --help` repeats this setup.

### 1. Google Cloud — OAuth Desktop client

1. Create or pick a [Google Cloud project](https://console.cloud.google.com/).
2. Enable the [Google Ads API](https://console.cloud.google.com/apis/library/googleads.googleapis.com) on that project.
3. **APIs & Services → OAuth consent screen**
   - User type **External** is fine for your own account.
   - Add yourself as a **test user** while the app is unverified.
   - Add scope `https://www.googleapis.com/auth/adwords`.
4. **APIs & Services → Credentials → Create credentials → OAuth client ID**
   - Application type: **Desktop app** (not Web, not Chrome).
   - Download the JSON. Copy `client_id` and `client_secret`.
   - Desktop clients already allow `http://127.0.0.1` as a redirect. Do not register a port.

If the consent screen shows an unverified-app warning, click **Advanced → Go to &lt;project&gt; (unsafe)**.

### 2. Google Ads — developer token

1. Sign in to a Google Ads **manager** account (MCC).
2. **Admin → API Center** (or search “API Center”).
3. Apply for / copy the [developer token](https://developers.google.com/google-ads/api/docs/get-started/dev-token) (22-character string).
4. A **test-access** token only works against test accounts. Production accounts need **Basic** or higher.

`adscli login` does **not** send this token. Every later command (`customers`, `campaigns`, …) does.

### 3. Configure adscli

Either export:

```sh
export ADSCLI_CLIENT_ID=....apps.googleusercontent.com
export ADSCLI_CLIENT_SECRET=...
export ADSCLI_DEVELOPER_TOKEN=...
```

or write `~/.config/adscli/config.yaml` or `.adscli.yaml` in the project (Python `google-ads.yaml` field names work too):

```yaml
developer_token: "..."
client_id: "....apps.googleusercontent.com"
client_secret: "..."
# set after login, or export them:
# customer_id: "123-456-7890"
# login_customer_id: "098-765-4321"   # MCC, if you use one
```

### 4. Sign in

```sh
adscli login                    # opens the browser (PKCE + loopback)
# adscli login --device         # no local HTTP server
# adscli login --print-url      # print the URL, do not open a browser
adscli auth status --json       # must show has_refresh_token and has_developer_token
```

The refresh token is stored in the OS keychain, or in `~/.config/adscli/credentials.json` (mode `0600`) if the keychain is unavailable. Later commands refresh the access token silently.

As of August 2026, Google may require a **passkey** when issuing a **new** Ads refresh token. Existing tokens keep working.

### 5. Pick an account

```sh
adscli customers list --json
export ADSCLI_CUSTOMER_ID=1234567890          # dashes ok
export ADSCLI_LOGIN_CUSTOMER_ID=0987654321    # required when the user sits under an MCC
adscli campaigns list --json --limit 5
```

### Agents

Do not run `adscli login` from a script. After a human has logged in once, set `ADSCLI_REFRESH_TOKEN` (and the client id/secret + developer token) or point `--config` at the YAML above. Check `adscli auth status --json`.

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
