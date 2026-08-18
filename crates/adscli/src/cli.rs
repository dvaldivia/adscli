use std::path::PathBuf;
use std::process::ExitCode;

use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};

use crate::commands;

const LONG_ABOUT: &str = "\
adscli is an agent-first CLI for the Google Ads API (v25).

DISCOVERY (do this first):
  adscli --help
  adscli schema --json          dump the full command tree + flags
  adscli <command> --help       every command documents flags and examples
  adscli version --json         confirm the binary and API version

HEADLESS / AGENT MODE:
  All subcommands exit. None of them prompt. Add --json for structured
  stdout (errors go to stderr as JSON too). Mutations accept --dry-run
  and require --yes when they spend money or delete.

  Exit codes: 0 ok, 1 error, 2 usage, 3 not found, 4 auth, 5 conflict.

INTERACTIVE TUI (default, requires a TTY):
  Run with no subcommand to browse campaigns → asset groups → assets
  like k9s. In a pipe or script this exits immediately and tells you
  which subcommand to use instead.

CONFIGURATION (flag > env > file > built-in Desktop client):
  ADSCLI_DEVELOPER_TOKEN   required for every API call
  ADSCLI_CUSTOMER_ID       10-digit account (dashes ok)
  ADSCLI_LOGIN_CUSTOMER_ID MCC / manager account
  ADSCLI_CLIENT_ID / ADSCLI_CLIENT_SECRET   optional; override the shared client
  ADSCLI_REFRESH_TOKEN
  Config file: --config, ./.adscli.yaml, ~/.config/adscli/config.yaml

SSO LOGIN:
  adscli login                 open a browser (Desktop OAuth + PKCE)
  adscli login --device        device-code flow (no local HTTP server)
  adscli auth status --json    check tokens without secrets
  adscli logout                drop the keychain entry and credentials file
";

#[derive(Debug, Parser)]
#[command(
    name = "adscli",
    about = "Agent-first CLI and k9s-style TUI for the Google Ads API v25",
    long_about = LONG_ABOUT,
    disable_help_subcommand = true,
    arg_required_else_help = false,
    after_help = "Examples:\n  adscli --help\n  adscli login\n  adscli schema --json\n  adscli campaigns list --json --limit 20\n  adscli performance campaigns --during LAST_7_DAYS --json\n  adscli campaigns pause 123 --yes --json"
)]
pub struct Cli {
    /// path to YAML config (default: .adscli.yaml, then ~/.config/adscli/config.yaml)
    #[arg(long, global = true, env = "ADSCLI_CONFIG")]
    pub config: Option<PathBuf>,

    /// emit machine-readable JSON instead of a table
    #[arg(long, global = true)]
    pub json: bool,

    /// print only identifiers, one per line (list commands)
    #[arg(long, short = 'q', global = true)]
    pub quiet: bool,

    /// Google Ads customer id (dashes optional). Env: ADSCLI_CUSTOMER_ID
    #[arg(long, global = true, env = "ADSCLI_CUSTOMER_ID")]
    pub customer_id: Option<String>,

    /// manager (MCC) customer id sent as login-customer-id
    #[arg(long, global = true, env = "ADSCLI_LOGIN_CUSTOMER_ID")]
    pub login_customer_id: Option<String>,

    /// Google Ads developer token
    #[arg(long, global = true, env = "ADSCLI_DEVELOPER_TOKEN")]
    pub developer_token: Option<String>,

    /// OAuth client id (Desktop app). Defaults to the shared adscli client.
    #[arg(long, global = true, env = "ADSCLI_CLIENT_ID")]
    pub client_id: Option<String>,

    /// OAuth client secret. Defaults to the shared adscli Desktop secret.
    #[arg(long, global = true, env = "ADSCLI_CLIENT_SECRET")]
    pub client_secret: Option<String>,

    /// OAuth refresh token
    #[arg(long, global = true, env = "ADSCLI_REFRESH_TOKEN")]
    pub refresh_token: Option<String>,

    /// REST origin including version, default https://googleads.googleapis.com/v25
    #[arg(long, global = true, env = "ADSCLI_API_BASE")]
    pub api_base: Option<String>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Print version, OS, arch, rustc, and API version
    #[command(after_help = "Examples:\n  adscli version\n  adscli version --json")]
    Version,

    /// Dump the full command tree as JSON (agent discovery)
    #[command(
        long_about = "Walk the clap command tree and emit name, about, args, and nested commands.\nAgents should call this once instead of scraping --help.",
        after_help = "Examples:\n  adscli schema --json"
    )]
    Schema,

    /// Browser SSO login (Desktop OAuth + PKCE). Alias of `auth login`
    #[command(
        long_about = LOGIN_ABOUT,
        after_help = "Examples:\n  adscli login\n  adscli login --print-url\n  adscli login --device\n  adscli login --no-browser --port 8080"
    )]
    Login {
        #[command(flatten)]
        opts: LoginOpts,
    },

    /// Delete the cached refresh token (keychain + credentials file)
    #[command(after_help = "Examples:\n  adscli logout\n  adscli logout --json")]
    Logout,

    /// OAuth login, status, and logout
    #[command(
        after_help = "Examples:\n  adscli auth status --json\n  adscli login\n  adscli auth login --device\n  adscli logout"
    )]
    Auth {
        #[command(subcommand)]
        command: AuthCmd,
    },

    /// Show resolved config with secrets redacted
    #[command(after_help = "Examples:\n  adscli config show --json\n  adscli config path")]
    Config {
        #[command(subcommand)]
        command: ConfigCmd,
    },

    /// List accessible Google Ads accounts
    #[command(
        after_help = "Examples:\n  adscli customers list --json\n  adscli customers get --json"
    )]
    Customers {
        #[command(subcommand)]
        command: CustomersCmd,
    },

    /// Browse, create, update, pause, enable, or remove campaigns
    #[command(
        after_help = "Examples:\n  adscli campaigns list --json --limit 20\n  adscli campaigns get 123 --json\n  adscli campaigns pause 123 --yes --json\n  adscli campaigns create --name 'PMax' --channel-type PERFORMANCE_MAX --budget-micros 10000000 --dry-run --json"
    )]
    Campaigns {
        #[command(subcommand)]
        command: CampaignsCmd,
    },

    /// Browse and mutate Performance Max asset groups
    #[command(
        name = "asset-groups",
        after_help = "Examples:\n  adscli asset-groups list --campaign 123 --json\n  adscli asset-groups create --campaign 123 --name Homepage --final-url https://example.com --dry-run --json"
    )]
    AssetGroups {
        #[command(subcommand)]
        command: AssetGroupsCmd,
    },

    /// Browse, create, and link assets
    #[command(
        after_help = "Examples:\n  adscli assets list --json\n  adscli assets create --type TEXT --text 'Free shipping' --json --dry-run\n  adscli assets link --asset-group 20 --asset 99 --field-type HEADLINE --yes --json"
    )]
    Assets {
        #[command(subcommand)]
        command: AssetsCmd,
    },

    /// Performance metrics for campaigns, asset groups, or assets
    #[command(
        after_help = "Examples:\n  adscli performance campaigns --during LAST_7_DAYS --json\n  adscli performance asset-groups --campaign 123 --json\n  adscli performance assets --asset-group 20 --json"
    )]
    Performance {
        #[command(subcommand)]
        command: PerformanceCmd,
    },

    /// Run a raw GAQL query (power-user / agent escape hatch)
    #[command(
        long_about = "Execute Google Ads Query Language against GoogleAdsService.Search.\nUse this when a first-class command does not expose a field. Prefer --json.",
        after_help = "Examples:\n  adscli gaql --query \"SELECT campaign.id, campaign.name FROM campaign LIMIT 5\" --json"
    )]
    Gaql {
        /// GAQL string
        #[arg(long, short = 'Q')]
        query: String,
    },
}

const LOGIN_ABOUT: &str = "\
Sign in to Google Ads with a browser (OAuth 2.0 Desktop / Installed app).

DEFAULT CLIENT (compiled into every binary):
  client_id     REDACTED
  client_secret REDACTED

  You do not need to export those. adscli login uses them unless
  overridden. The developer token is NOT shipped — set
  ADSCLI_DEVELOPER_TOKEN (or config.yaml) for API calls.

  Resolution, highest wins:
    --client-id / --client-secret
    ADSCLI_CLIENT_ID / ADSCLI_CLIENT_SECRET
    client_id / client_secret in config.yaml or credentials.json
    the compiled defaults above

  When the defaults win: `adscli auth status --json` and
  `adscli config show --json` report oauth_from_bundle=true and
  has_oauth_client=true. Secrets are never printed.

ONE SHARED CLIENT (yes, branding is for this):
  The Cloud OAuth consent-screen branding (app name, logo, homepage
  https://adscli.dev, privacy https://adscli.dev/privacy.html) is
  what users see when they click Allow. Every adscli user shares
  this Desktop client. They still sign in as themselves; they do
  not create their own Cloud project.

  Until Google verifies the app, only listed test users can finish
  the consent screen. Unverified users see Advanced → Go to <app>.

  Desktop client_secret is public-by-design (it ships in the
  binary; Google assumes installed apps cannot keep secrets).
  Extracting it does not grant anyone's Ads account — a human
  must still click Allow. PKCE still protects the loopback code
  exchange. A leaked developer token is worse: it is the app's
  API permit and quota.

BRING YOUR OWN CLIENT (a different Cloud project):
  1. Google Cloud project + enable Google Ads API
  2. OAuth consent screen, scope https://www.googleapis.com/auth/adwords
  3. Credentials → OAuth client ID → Desktop app
  4. Google Ads Admin → API Center → developer token
  5. export ADSCLI_CLIENT_ID ADSCLI_CLIENT_SECRET ADSCLI_DEVELOPER_TOKEN
     adscli login

  Desktop clients already allow http://127.0.0.1; do not register a port.
  New refresh tokens (as of August 2026) may ask for a passkey.

WHAT HAPPENS:
  1. adscli binds http://127.0.0.1 (loopback) and builds a consent URL
     with scope https://www.googleapis.com/auth/adwords, access_type=offline,
     PKCE (S256), and a CSRF state parameter.
  2. The default browser opens (or the URL is printed).
  3. After you approve, Google redirects to the loopback listener.
  4. adscli exchanges the code for an access token + refresh token.
  5. The refresh token is stored in the OS keychain when available,
     otherwise in credentials.json with mode 0600.

AGENTS:
  Do not call login from a script. Set ADSCLI_REFRESH_TOKEN and
  ADSCLI_DEVELOPER_TOKEN (client id/secret only to override the
  built-in client). Use `adscli auth status --json` to inspect
  whether credentials are present. Use --device on a machine with
  no browser; use --code to exchange a code obtained elsewhere.
";

#[derive(Debug, Clone, clap::Args)]
pub struct LoginOpts {
    /// print the consent URL and do not open a browser
    #[arg(long)]
    pub print_url: bool,
    /// do not open a browser (still waits on the loopback redirect)
    #[arg(long)]
    pub no_browser: bool,
    /// OAuth device-code flow — no local HTTP server; for machines without a browser
    #[arg(long)]
    pub device: bool,
    /// loopback TCP port (default: ephemeral unused port)
    #[arg(long)]
    pub port: Option<u16>,
    /// skip the listener and exchange this authorization code
    #[arg(long)]
    pub code: Option<String>,
    /// redirect URI that matches the OAuth client (required with --code)
    #[arg(long)]
    pub redirect_uri: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum AuthCmd {
    /// Open a browser, capture the OAuth code, store a refresh token
    #[command(
        long_about = LOGIN_ABOUT,
        after_help = "Examples:\n  adscli auth login\n  adscli login --print-url\n  adscli login --device\n  adscli auth login --code AUTH_CODE --redirect-uri http://127.0.0.1:PORT"
    )]
    Login {
        #[command(flatten)]
        opts: LoginOpts,
    },
    /// Show whether tokens, developer token, and the bundled OAuth client are configured (no secrets)
    Status,
    /// Delete the cached credentials file and keychain entry
    Logout,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCmd {
    /// Print the config and credentials file paths
    Path,
    /// Print resolved settings with secrets redacted (`oauth_from_bundle` is true when the built-in Desktop client is in use)
    Show,
}

#[derive(Debug, Subcommand)]
pub enum CustomersCmd {
    /// List accounts the OAuth user can access
    List,
    /// Get the current --customer-id account
    Get,
}

#[derive(Debug, Clone, clap::Args)]
pub struct ListOpts {
    /// ENABLED, PAUSED, or REMOVED
    #[arg(long)]
    pub status: Option<String>,
    /// max rows (0 = no limit). Default 50 to keep agent context small
    #[arg(long, default_value_t = 50)]
    pub limit: usize,
    /// order by field, e.g. cost, impressions, name, cost desc
    #[arg(long)]
    pub order_by: Option<String>,
    /// include metrics (uses --during / --from / --to)
    #[arg(long, default_value_t = true)]
    pub metrics: bool,
    /// skip metrics (faster, no date range)
    #[arg(long)]
    pub no_metrics: bool,
    /// GAQL DURING preset (default LAST_30_DAYS)
    #[arg(long)]
    pub during: Option<String>,
    /// start date YYYY-MM-DD (use with --to)
    #[arg(long)]
    pub from: Option<String>,
    /// end date YYYY-MM-DD (use with --from)
    #[arg(long)]
    pub to: Option<String>,
    /// substring match on name
    #[arg(long)]
    pub name_contains: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum CampaignsCmd {
    /// List campaigns (metrics last 30 days by default)
    List {
        #[command(flatten)]
        opts: ListOpts,
        /// advertising channel type filter is not sent; use gaql for that
        #[arg(long)]
        campaign: Option<String>,
    },
    /// Get one campaign by id or resource name
    Get { id: String },
    /// Create a campaign (PAUSED by default) plus a daily budget
    Create {
        #[arg(long)]
        name: String,
        /// SEARCH, PERFORMANCE_MAX, DISPLAY, DEMAND_GEN, VIDEO
        #[arg(long, default_value = "SEARCH")]
        channel_type: String,
        /// ENABLED or PAUSED (default PAUSED so creates never spend)
        #[arg(long, default_value = "PAUSED")]
        status: String,
        /// daily budget in micros (1_000_000 = 1.00 in account currency)
        #[arg(long)]
        budget_micros: i64,
        /// reuse an existing campaign budget resource name
        #[arg(long)]
        budget_resource: Option<String>,
        /// maximize_conversions | maximize_conversion_value | maximize_clicks | target_cpa | target_roas | manual_cpc
        #[arg(long, default_value = "maximize_conversions")]
        bidding: String,
        #[arg(long)]
        target_cpa_micros: Option<i64>,
        #[arg(long)]
        target_roas: Option<f64>,
        #[arg(long, default_value = "DOES_NOT_CONTAIN_EU_POLITICAL_ADVERTISING")]
        eu_political: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
    },
    /// Update name and/or status
    Update {
        id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
    },
    /// Set status ENABLED
    Enable {
        id: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
    },
    /// Set status PAUSED
    Pause {
        id: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
    },
    /// Set status REMOVED
    Remove {
        id: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum AssetGroupsCmd {
    List {
        #[command(flatten)]
        opts: ListOpts,
        /// parent campaign id or resource name
        #[arg(long)]
        campaign: Option<String>,
    },
    Get {
        id: String,
    },
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        campaign: String,
        #[arg(long, default_value = "PAUSED")]
        status: String,
        /// repeatable final URL
        #[arg(long = "final-url")]
        final_urls: Vec<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
    },
    Update {
        id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long = "final-url")]
        final_urls: Vec<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
    },
    Enable {
        id: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
    },
    Pause {
        id: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
    },
    Remove {
        id: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum AssetsCmd {
    List {
        #[command(flatten)]
        opts: ListOpts,
        #[arg(long)]
        campaign: Option<String>,
        #[arg(long)]
        asset_group: Option<String>,
        /// TEXT, IMAGE, YOUTUBE_VIDEO, ...
        #[arg(long = "type")]
        asset_type: Option<String>,
        /// list links (asset_group_asset) instead of the account asset library
        #[arg(long)]
        links: bool,
    },
    Get {
        id: String,
    },
    Create {
        /// TEXT, IMAGE, YOUTUBE_VIDEO
        #[arg(long = "type")]
        asset_type: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        text: Option<String>,
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long)]
        youtube_id: Option<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
    },
    Update {
        id: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
    },
    /// Attach an existing asset to an asset group
    Link {
        #[arg(long)]
        asset_group: String,
        #[arg(long)]
        asset: String,
        /// HEADLINE, DESCRIPTION, LONG_HEADLINE, BUSINESS_NAME, MARKETING_IMAGE, ...
        #[arg(long)]
        field_type: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
    },
    /// Remove an asset-group link (does not delete the asset)
    Unlink {
        #[arg(long)]
        asset_group: String,
        #[arg(long)]
        asset: String,
        #[arg(long)]
        field_type: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum PerformanceCmd {
    Campaigns {
        #[command(flatten)]
        opts: ListOpts,
    },
    #[command(name = "asset-groups")]
    AssetGroups {
        #[command(flatten)]
        opts: ListOpts,
        #[arg(long)]
        campaign: Option<String>,
    },
    Assets {
        #[command(flatten)]
        opts: ListOpts,
        #[arg(long)]
        campaign: Option<String>,
        #[arg(long)]
        asset_group: Option<String>,
    },
}

fn invocation_name() -> String {
    let argv0 = std::env::args().next().unwrap_or_default();
    std::path::Path::new(&argv0)
        .file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty() && *s != "." && *s != "/")
        .unwrap_or("adscli")
        .to_string()
}

pub fn run() -> ExitCode {
    let name: &'static str = Box::leak(invocation_name().into_boxed_str());
    let cmd = Cli::command().name(name).bin_name(name);
    let matches = cmd.get_matches();
    let cli = match Cli::from_arg_matches(&matches) {
        Ok(c) => c,
        Err(e) => e.exit(),
    };

    match cli.command {
        Some(Command::Version) => commands::version::run(cli.json),
        Some(Command::Schema) => commands::schema::run(cli.json),
        Some(Command::Login { ref opts }) => commands::auth::login(&cli, opts),
        Some(Command::Logout) => commands::auth::logout(&cli),
        Some(Command::Auth { ref command }) => commands::auth::run(&cli, command),
        Some(Command::Config { ref command }) => commands::config::run(&cli, command),
        Some(Command::Customers { ref command }) => commands::customers::run(&cli, command),
        Some(Command::Campaigns { ref command }) => commands::campaigns::run(&cli, command),
        Some(Command::AssetGroups { ref command }) => commands::asset_groups::run(&cli, command),
        Some(Command::Assets { ref command }) => commands::assets::run(&cli, command),
        Some(Command::Performance { ref command }) => commands::performance::run(&cli, command),
        Some(Command::Gaql { ref query }) => commands::gaql::run(&cli, query),
        None => commands::tui::run(&cli),
    }
}

pub fn filter_from(opts: &ListOpts) -> Result<adscli_api::query::ListFilter, adscli_api::ApiError> {
    let with_metrics = opts.metrics && !opts.no_metrics;
    let date_range = if with_metrics {
        Some(adscli_api::DateRange::parse(
            opts.during.as_deref(),
            opts.from.as_deref(),
            opts.to.as_deref(),
        )?)
    } else {
        None
    };
    Ok(adscli_api::query::ListFilter {
        status: opts.status.clone(),
        name_contains: opts.name_contains.clone(),
        limit: Some(opts.limit),
        order_by: opts.order_by.clone(),
        with_metrics,
        date_range,
        ..Default::default()
    })
}
