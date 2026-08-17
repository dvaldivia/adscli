//! Customer IDs and resource-name helpers.
//!
//! Google Ads displays customer IDs as `123-456-7890`. The REST API wants
//! digits only. Callers may pass either form, a bare resource id, or a full
//! `customers/{cid}/campaigns/{id}` name.

/// Strip dashes / spaces and any `customers/` prefix. Non-digit characters
/// other than those are dropped so `customers/123-456-7890` works.
pub fn normalize_customer_id(raw: &str) -> String {
    let s = raw.trim();
    let s = s
        .strip_prefix("customers/")
        .or_else(|| s.strip_prefix("customers:"))
        .unwrap_or(s);
    s.chars().filter(|c| c.is_ascii_digit()).collect()
}

/// Last path segment of a resource name, or the input if it has no slash.
pub fn extract_resource_id(name_or_id: &str) -> String {
    name_or_id
        .trim()
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(name_or_id)
        .to_string()
}

/// Drop a leading `customers/{id}/` so `campaigns/5` stays `campaigns/5`.
pub fn strip_customer_prefix(resource_name: &str) -> &str {
    let s = resource_name.trim();
    if let Some(rest) = s.strip_prefix("customers/") {
        if let Some(idx) = rest.find('/') {
            return &rest[idx + 1..];
        }
        return "";
    }
    s
}

/// Build `customers/{cid}/{collection}/{id}` unless `id_or_name` is already
/// a full resource name.
pub fn resource_name(customer_id: &str, collection: &str, id_or_name: &str) -> String {
    let t = id_or_name.trim();
    if t.starts_with("customers/") {
        return t.to_string();
    }
    let cid = normalize_customer_id(customer_id);
    let id = extract_resource_id(t);
    format!("customers/{cid}/{collection}/{id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_dashed_and_prefixed() {
        assert_eq!(normalize_customer_id("123-456-7890"), "1234567890");
        assert_eq!(
            normalize_customer_id("customers/123-456-7890"),
            "1234567890"
        );
        assert_eq!(normalize_customer_id("  1234567890  "), "1234567890");
    }

    #[test]
    fn extracts_id() {
        assert_eq!(extract_resource_id("customers/1/campaigns/99"), "99");
        assert_eq!(extract_resource_id("99"), "99");
    }

    #[test]
    fn builds_resource_name() {
        assert_eq!(
            resource_name("123-456-7890", "campaigns", "99"),
            "customers/1234567890/campaigns/99"
        );
        assert_eq!(
            resource_name("1", "campaigns", "customers/1/campaigns/99"),
            "customers/1/campaigns/99"
        );
    }
}
