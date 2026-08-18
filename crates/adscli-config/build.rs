fn main() {
    // Optional compile-time overrides of the source-level Desktop client
    // (and an optional bundled developer token). Local builds use the
    // defaults in bundled.rs when these are unset.
    for key in [
        "ADSCLI_BUNDLED_CLIENT_ID",
        "ADSCLI_BUNDLED_CLIENT_SECRET",
        "ADSCLI_BUNDLED_DEVELOPER_TOKEN",
    ] {
        println!("cargo:rerun-if-env-changed={key}");
        if let Ok(v) = std::env::var(key)
            && !v.trim().is_empty()
        {
            println!("cargo:rustc-env={key}={v}");
        }
    }
}
