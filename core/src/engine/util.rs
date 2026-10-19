use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn b64(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD_NO_PAD.encode(bytes)
}

pub fn unb64(s: &str) -> anyhow::Result<Vec<u8>> {
    use base64::Engine;
    let s = s.trim_end_matches('=');
    base64::engine::general_purpose::STANDARD_NO_PAD
        .decode(s)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(s))
        .map_err(|e| anyhow::anyhow!("invalid base64: {e}"))
}
