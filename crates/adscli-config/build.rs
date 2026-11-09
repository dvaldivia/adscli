fn main() {
    // Optional compile-time Desktop OAuth client. Local builds leave
    // these unset and stay bring-your-own.
    for key in ["ADSCLI_BUNDLED_CLIENT_ID", "ADSCLI_BUNDLED_CLIENT_SECRET"] {
        println!("cargo:rerun-if-env-changed={key}");
        if let Ok(v) = std::env::var(key)
            && !v.trim().is_empty()
        {
            println!("cargo:rustc-env={key}={v}");
        }
    }
}
