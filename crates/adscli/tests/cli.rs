use assert_cmd::Command;
use predicates::prelude::*;

fn ads() -> Command {
    Command::cargo_bin("adscli").unwrap()
}

#[test]
fn help_lists_nouns_and_mentions_json() {
    ads()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("campaigns"))
        .stdout(predicate::str::contains("asset-groups"))
        .stdout(predicate::str::contains("assets"))
        .stdout(predicate::str::contains("performance"))
        .stdout(predicate::str::contains("schema"))
        .stdout(predicate::str::contains("--json"))
        .stdout(predicate::str::contains("schema --json"))
        .stdout(predicate::str::contains("login"));
}

#[test]
fn campaigns_help_has_examples_and_dry_run() {
    ads()
        .args(["campaigns", "create", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--budget-micros"))
        .stdout(predicate::str::contains("--dry-run"))
        .stdout(predicate::str::contains("--yes"))
        .stdout(predicate::str::contains("PAUSED"));
}

#[test]
fn version_json_shape() {
    let out = ads().args(["version", "--json"]).assert().success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout);
    assert!(stdout.ends_with('\n'));
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let obj = v.as_object().unwrap();
    for k in ["api_version", "arch", "os", "runtime", "version"] {
        assert!(obj.contains_key(k), "missing {k} in {stdout}");
    }
    assert_eq!(obj["api_version"], "v26");
}

#[test]
fn schema_json_contains_campaigns() {
    let out = ads().args(["schema", "--json"]).assert().success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let names: Vec<String> = v["commands"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|c| c["name"].as_str().map(str::to_string))
        .collect();
    for want in [
        "version",
        "schema",
        "login",
        "logout",
        "auth",
        "customers",
        "campaigns",
        "asset-groups",
        "assets",
        "performance",
        "gaql",
    ] {
        assert!(
            names.iter().any(|n| n == want),
            "missing {want} in {names:?}"
        );
    }
}

#[test]
fn login_help_documents_pkce_and_device() {
    ads()
        .args(["login", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("PKCE"))
        .stdout(predicate::str::contains("adwords"))
        .stdout(predicate::str::contains("--device"))
        .stdout(predicate::str::contains("keychain"))
        .stdout(predicate::str::contains("passkey"))
        .stdout(predicate::str::contains("ONE SHARED CLIENT"))
        .stdout(predicate::str::contains("Desktop app"))
        .stdout(predicate::str::contains("oauth_from_bundle"))
        .stdout(predicate::str::contains(
            "CLOUD_PROJECT_NOT_APPROVED_FOR_PRODUCTION",
        ))
        .stdout(predicate::str::contains("Explorer"))
        .stdout(predicate::str::contains("ADSCLI_BUNDLED_CLIENT_ID"))
        .stdout(predicate::str::contains("paste"));
}

#[test]
fn source_build_login_needs_client_unless_bundled() {
    if adscli_config::has_bundled_oauth() {
        return;
    }
    ads()
        .env_remove("ADSCLI_CLIENT_ID")
        .env_remove("ADSCLI_CLIENT_SECRET")
        .env("HOME", "/tmp/adscli-empty-home-login")
        .env("ADSCLI_FORCE_FILE_STORE", "1")
        .args(["login", "--json"])
        .assert()
        .failure()
        .code(4)
        .stderr(predicate::str::contains("client_id"));
}

#[test]
fn no_tty_default_exits_usage() {
    ads()
        .env("ADSCLI_DEVELOPER_TOKEN", "x")
        .env("ADSCLI_ACCESS_TOKEN", "x")
        .env("ADSCLI_SKIP_TOKEN_REFRESH", "1")
        .env("ADSCLI_CUSTOMER_ID", "123")
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("no terminal detected"));
}

#[test]
fn missing_customer_id_is_usage() {
    let mut server = mockito::Server::new();
    let _m = server
        .mock("POST", "/v26/customers/googleAds:search")
        .with_status(200)
        .with_body("{}")
        .create();

    ads()
        .env("ADSCLI_DEVELOPER_TOKEN", "x")
        .env("ADSCLI_ACCESS_TOKEN", "x")
        .env("ADSCLI_SKIP_TOKEN_REFRESH", "1")
        .env("ADSCLI_API_BASE", format!("{}/v26", server.url()))
        .args(["campaigns", "list", "--json", "--no-metrics"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("customer id"));
}

#[test]
fn campaigns_list_json_against_mock() {
    let mut server = mockito::Server::new();
    let body = r#"{
      "results": [
        {
          "campaign": {
            "resourceName": "customers/123/campaigns/9",
            "id": "9",
            "name": "Brand",
            "status": "ENABLED",
            "advertisingChannelType": "SEARCH"
          },
          "campaignBudget": { "amountMicros": "5000000" }
        }
      ]
    }"#;
    // OAuth only: developer tokens were sunset, so none is configured
    // and the header must not be sent.
    let m = server
        .mock("POST", "/v26/customers/123/googleAds:search")
        .match_header("authorization", "Bearer x")
        .match_header("developer-token", mockito::Matcher::Missing)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body)
        .create();

    let out = ads()
        .env_remove("ADSCLI_DEVELOPER_TOKEN")
        .env("ADSCLI_CONFIG", "/tmp/adscli-empty-home/none.yaml")
        .env("ADSCLI_ACCESS_TOKEN", "x")
        .env("ADSCLI_SKIP_TOKEN_REFRESH", "1")
        .env("ADSCLI_CUSTOMER_ID", "123")
        .env("ADSCLI_API_BASE", format!("{}/v26", server.url()))
        .args([
            "campaigns",
            "list",
            "--json",
            "--no-metrics",
            "--limit",
            "10",
        ])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v[0]["id"], "9");
    assert_eq!(v[0]["name"], "Brand");
    assert_eq!(v[0]["status"], "ENABLED");
    m.assert();
}

#[test]
fn pause_requires_yes() {
    ads()
        .env("ADSCLI_DEVELOPER_TOKEN", "x")
        .env("ADSCLI_ACCESS_TOKEN", "x")
        .env("ADSCLI_SKIP_TOKEN_REFRESH", "1")
        .env("ADSCLI_CUSTOMER_ID", "123")
        .args(["campaigns", "pause", "9", "--json"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("--yes"));
}

#[test]
fn pause_dry_run_json() {
    ads()
        .env("ADSCLI_DEVELOPER_TOKEN", "x")
        .env("ADSCLI_ACCESS_TOKEN", "x")
        .env("ADSCLI_SKIP_TOKEN_REFRESH", "1")
        .env("ADSCLI_CUSTOMER_ID", "123")
        .args(["campaigns", "pause", "9", "--dry-run", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"dry_run\": true"));
}

#[test]
fn customers_list_mock() {
    let mut server = mockito::Server::new();
    // A legacy developer token is forwarded when one is configured.
    let m = server
        .mock("GET", "/v26/customers:listAccessibleCustomers")
        .match_header("developer-token", "x")
        .with_status(200)
        .with_body(r#"{"resourceNames":["customers/1234567890"]}"#)
        .create();

    let out = ads()
        .env("ADSCLI_DEVELOPER_TOKEN", "x")
        .env("ADSCLI_ACCESS_TOKEN", "x")
        .env("ADSCLI_SKIP_TOKEN_REFRESH", "1")
        .env("ADSCLI_API_BASE", format!("{}/v26", server.url()))
        .args(["customers", "list", "--json"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v[0]["id"], "1234567890");
    m.assert();
}

#[test]
fn json_error_shape() {
    ads()
        .env_remove("ADSCLI_DEVELOPER_TOKEN")
        .env_remove("ADSCLI_ACCESS_TOKEN")
        .env_remove("ADSCLI_REFRESH_TOKEN")
        .env("ADSCLI_FORCE_FILE_STORE", "1")
        .env("ADSCLI_CONFIG", "/tmp/adscli-empty-home/none.yaml")
        .env("ADSCLI_CUSTOMER_ID", "123")
        .env("HOME", "/tmp/adscli-empty-home")
        .args(["campaigns", "list", "--json", "--no-metrics"])
        .assert()
        .failure()
        .code(4)
        .stderr(predicate::str::contains("\"error\""))
        .stderr(predicate::str::contains("adscli auth login"));
}

#[test]
fn test_access_project_error_explains_access_levels() {
    let mut server = mockito::Server::new();
    let body = r#"{
      "error": {
        "code": 403,
        "message": "The caller does not have permission",
        "status": "PERMISSION_DENIED",
        "details": [{
          "@type": "type.googleapis.com/google.ads.googleads.v26.errors.GoogleAdsFailure",
          "errors": [{
            "errorCode": {"authorizationError": "CLOUD_PROJECT_NOT_APPROVED_FOR_PRODUCTION"},
            "message": "The Cloud project is only approved for use with test accounts."
          }]
        }]
      }
    }"#;
    let _m = server
        .mock("POST", "/v26/customers/123/googleAds:search")
        .with_status(403)
        .with_header("content-type", "application/json")
        .with_body(body)
        .create();

    ads()
        .env_remove("ADSCLI_DEVELOPER_TOKEN")
        .env("ADSCLI_CONFIG", "/tmp/adscli-empty-home/none.yaml")
        .env("ADSCLI_ACCESS_TOKEN", "x")
        .env("ADSCLI_SKIP_TOKEN_REFRESH", "1")
        .env("ADSCLI_CUSTOMER_ID", "123")
        .env("ADSCLI_API_BASE", format!("{}/v26", server.url()))
        .args(["campaigns", "list", "--json", "--no-metrics"])
        .assert()
        .failure()
        .code(4)
        .stderr(predicate::str::contains(
            "CLOUD_PROJECT_NOT_APPROVED_FOR_PRODUCTION",
        ))
        .stderr(predicate::str::contains("Explorer access"));
}
