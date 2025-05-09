use base64::{engine::general_purpose, Engine as _};
use tracing::debug;

/// Base64エンコードを行う関数
pub fn base64_encode(input: &str) -> String {
    // デバッグ情報を出力
    debug!("Base64 encoding input string length: {}", input.len());
    debug!("Base64 encoding input: '{}'", input);

    // STANDARDエンコーダを使用 (パディングあり)
    let encoded = general_purpose::STANDARD.encode(input.as_bytes());

    debug!("Base64 encoded result length: {}", encoded.len());
    debug!("Base64 encoded result: '{}'", encoded);

    encoded
}

/// Base64デコードを行う関数
pub fn base64_decode(input: &str) -> Result<String, base64::DecodeError> {
    let bytes = general_purpose::STANDARD.decode(input)?;
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

/// CouchDB URLから認証情報を抽出する関数
pub fn extract_auth_from_url(url: &str) -> Option<(String, String)> {
    if let Ok(parsed_url) = url::Url::parse(url) {
        if !parsed_url.username().is_empty() {
            return Some((
                parsed_url.username().to_string(),
                parsed_url.password().unwrap_or("").to_string(),
            ));
        }
    }
    None
}

/// 指定された文字数だけ文字列を短縮し、残りを「...」で置き換える関数
pub fn truncate_string(s: &str, max_length: usize) -> String {
    if s.len() <= max_length {
        s.to_string()
    } else {
        format!("{}...", &s[..max_length])
    }
}
