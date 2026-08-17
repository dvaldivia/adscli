//! Walk Google Ads REST JSON. Proto fields are snake_case in GAQL and
//! camelCase on the wire; we accept both.

use serde_json::Value;

pub fn snake_to_camel(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut up = false;
    for c in s.chars() {
        if c == '_' {
            up = true;
        } else if up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

pub fn walk<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = v;
    for part in path.split('.') {
        if part.is_empty() {
            continue;
        }
        let camel = snake_to_camel(part);
        cur = cur
            .get(part)
            .or_else(|| cur.get(camel.as_str()))
            .or_else(|| {
                // REST sometimes nests the resource under its type name.
                None
            })?;
    }
    Some(cur)
}

pub fn as_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

pub fn as_i64(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64().or_else(|| n.as_u64().map(|u| u as i64)),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

pub fn as_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

pub fn get_string(row: &Value, path: &str) -> Option<String> {
    walk(row, path).and_then(as_string)
}

pub fn get_i64(row: &Value, path: &str) -> Option<i64> {
    walk(row, path).and_then(as_i64)
}

pub fn get_f64(row: &Value, path: &str) -> Option<f64> {
    walk(row, path).and_then(as_f64)
}

pub fn get_string_list(row: &Value, path: &str) -> Vec<String> {
    match walk(row, path) {
        Some(Value::Array(a)) => a.iter().filter_map(as_string).collect(),
        Some(v) => as_string(v).into_iter().collect(),
        None => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn camelizes() {
        assert_eq!(snake_to_camel("cost_micros"), "costMicros");
        assert_eq!(
            snake_to_camel("advertising_channel_type"),
            "advertisingChannelType"
        );
    }

    #[test]
    fn walks_camel_or_snake() {
        let v = json!({"metrics": {"costMicros": "12", "impressions": 3}});
        assert_eq!(get_i64(&v, "metrics.cost_micros"), Some(12));
        assert_eq!(get_i64(&v, "metrics.impressions"), Some(3));
    }
}
