# adscli

Agent-first CLI and k9s-style TUI for the [Google Ads API v26](https://developers.google.com/google-ads/api/docs/release-notes).

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
VERSION=0.0.4
OS=linux ARCH=x86_64
curl -sSL -o adscli.tar.gz \
  "https://github.com/dvaldivia/adscli/releases/download/v${VERSION}/adscli_${OS}_${ARCH}.tar.gz"
tar -xzf adscli.tar.gz
install -m 0755 adscli /usr/local/bin/adscli
```

`checksums.txt` in each release covers all four tarballs.

**Cargo install (from source):**

```sh
cargo install --git https://github.com/dvaldivia/adscli --tag v0.0.4 adscli
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

OAuth is the only credential. Google [sunset developer tokens](https://developers.google.com/google-ads/api/docs/get-started/dev-token) on 2026-09-09; you do not need one, and you do not need a manager account to get started.

Two things decide what you can do:

| Thing | Where it comes from | What it decides |
|---|---|---|
| Your sign-in (`adscli login`) | Your Google account, stored as a refresh token | *Which Ads accounts* you can reach. |
| The OAuth Desktop client (`client_id` + `client_secret`) | **Baked into official release binaries** (GitHub Actions secrets at compile time; not in git), or your own | *How much* you can do: the API access level and daily quota belong to the Google Cloud project that owns the client. |

Same facts live in `adscli login --help` and `adscli --help`.

### Official binaries (shared client)

Homebrew and GitHub Release builds compile in the adscli Desktop client from repo secrets (`ADSCLI_BUNDLED_CLIENT_ID` / `ADSCLI_BUNDLED_CLIENT_SECRET`). Those values are not checked in. First-time setup is:

```sh
adscli login
adscli auth status --json           # authenticated + oauth_from_bundle
adscli customers list --json
```

Client resolution, highest wins:

1. `--client-id` / `--client-secret`
2. `ADSCLI_CLIENT_ID` / `ADSCLI_CLIENT_SECRET`
3. `client_id` / `client_secret` in the config file or `credentials.json`
4. the compile-time bundle (official binaries only)

When (4) wins, `adscli auth status --json` and `adscli config show --json` report `has_oauth_client: true` and `oauth_from_bundle: true`. Those commands never print the secret.

Until Google [verifies](https://support.google.com/cloud/answer/9110914) the adscli consent screen, only **test users** listed on that screen can finish login. Everyone else hits the unverified-app wall (`Advanced → Go to adscli`). Branding (app name **adscli**, logo, [homepage](https://adscli.dev), [privacy policy](https://adscli.dev/privacy.html)) is what users see on the Allow page.

Google treats Desktop / installed apps as public clients: they [cannot keep secrets](https://developers.google.com/identity/protocols/oauth2/native-app). Anyone with an official binary can extract the client. That is expected. A stolen client secret is not a password for anyone’s Ads account, but it can spend the shared project’s daily quota. Rotate the Cloud secret and cut a new release if the client is abused.

### API access levels

The [access level](https://developers.google.com/google-ads/api/docs/api-policy/access-levels) is a property of the Cloud project that owns the OAuth client. It is not tied to you or to an Ads account.

| Level | Production accounts | Operations per day | How to get it |
|---|---|---|---|
| Test | no (test accounts only) | 15,000 | automatic when the Google Ads API is enabled |
| Explorer | yes | 2,880 | apply on the project’s API overview page |
| Basic | yes | 15,000 | apply; needs OAuth brand verification |
| Standard | yes | unlimited | apply; manual review, about 10 business days |

Everyone using the shared client draws on the same project quota. If you need your own, bring your own client.

A Test-level project that calls a production account gets `AuthorizationError.CLOUD_PROJECT_NOT_APPROVED_FOR_PRODUCTION` (exit code `4`). Logging in again does not help. The project owner applies for the next level on the [Google Ads API overview page](https://console.cloud.google.com/google/ads-apis/overview).

Explorer blocks account creation, user management, keyword/reach planning, and billing services. adscli does not use those.

### Bring your own client

Required for source builds (`cargo install`, `cargo build`), or to get your own quota from a **different** Google Cloud project.

1. Create or pick a [Google Cloud project](https://console.cloud.google.com/).
2. Enable the [Google Ads API](https://console.cloud.google.com/flows/enableapi?apiid=googleads.googleapis.com). The project now has Test access.
3. **APIs & Services → OAuth consent screen** — External is fine; add yourself as a test user while unverified; add scope `https://www.googleapis.com/auth/adwords`.
4. **Credentials → Create credentials → OAuth client ID → Desktop app** (not Web). Download the JSON. Desktop clients already allow `http://127.0.0.1`; do not register a port.
5. To use real (non-test) Ads accounts, open the [Google Ads API overview page](https://console.cloud.google.com/google/ads-apis/overview), expand **Upgrade access level**, and apply for Explorer.
6. Set the client (or override a release bundle):

```sh
export ADSCLI_CLIENT_ID=....apps.googleusercontent.com
export ADSCLI_CLIENT_SECRET=...
adscli login
adscli auth status --json    # oauth_from_bundle: false
```

or in `~/.config/adscli/config.yaml` / `.adscli.yaml` (Python `google-ads.yaml` field names work too):

```yaml
client_id: "....apps.googleusercontent.com"
client_secret: "..."
# customer_id: "123-456-7890"
# login_customer_id: "098-765-4321"   # MCC, only if you reach the account through one
```

### Sign in

```sh
adscli login                    # opens the browser (PKCE + loopback)
# adscli login --device         # no local HTTP server
# adscli login --print-url      # print the URL, do not open a browser
adscli auth status --json       # authenticated + has_refresh_token
adscli config show --json       # same facts, plus oauth_from_bundle
```

On SSH / a container / any host that is not your laptop, Google redirects to `127.0.0.1` on *your* machine, not the one running adscli. Leave `adscli login` waiting, copy the full `http://127.0.0.1:…/?state=…&code=…` URL from the browser address bar, paste it into that same process, and press Enter. A second `adscli login` cannot redeem that code (PKCE). `--device` is the other remote-friendly option.

The refresh token is stored in the OS keychain, or in `~/.config/adscli/credentials.json` (mode `0600`) if the keychain is unavailable. Later commands refresh the access token silently.

Google requires a **passkey** on the account that authorizes a **new** Ads refresh token, and a newly created passkey can take 7 days to become trusted. Existing tokens keep working.

### Pick an account

```sh
adscli customers list --json
export ADSCLI_CUSTOMER_ID=1234567890          # dashes ok
export ADSCLI_LOGIN_CUSTOMER_ID=0987654321    # only when you reach the account through an MCC
adscli campaigns list --json --limit 5
```

### Legacy developer token

`ADSCLI_DEVELOPER_TOKEN`, `--developer-token`, and `developer_token:` in the config file are still accepted. When set, adscli forwards the value as the `developer-token` header, which the API ignores. Google has said a future major API version will reject the header, so leave it unset for new setups.

### Agents

Do not run `adscli login` from a script. After a human has logged in once, set `ADSCLI_REFRESH_TOKEN` (client id/secret only if you are not using the built-in client) or point `--config` at the YAML above. Check `adscli auth status --json`.

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
