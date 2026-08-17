use std::collections::BTreeMap;
use std::process::ExitCode;

use adscli_config::API_VERSION;

use crate::platform;

pub fn run(json: bool) -> ExitCode {
    if json {
        let mut obj: BTreeMap<&str, &str> = BTreeMap::new();
        obj.insert("api_version", API_VERSION);
        obj.insert("arch", platform::arch());
        obj.insert("os", platform::os());
        obj.insert("runtime", platform::RUNTIME);
        obj.insert("version", platform::VERSION);
        match serde_json::to_string_pretty(&obj) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("Error: marshal JSON: {e}");
                return ExitCode::from(1);
            }
        }
    } else {
        println!(
            "adscli {} (google ads {}, {}/{}, {})",
            platform::VERSION,
            API_VERSION,
            platform::os(),
            platform::arch(),
            platform::RUNTIME,
        );
    }
    ExitCode::SUCCESS
}
