use chrono::Utc;
use encoding_rs::WINDOWS_1252;
use rand::distributions::{Alphanumeric, DistString};
use reqwest::blocking::Client;
use std::time::Duration;

const BASE_URL: &str = "https://www.bdciechi.it/route.php";
const IDEN_SP: &str = "SP";

#[derive(Clone)]
pub struct BdcQuota {
    pub remaining: String,
    pub monthly_total: String,
}

pub struct IdentifyResponse {
    pub nprov: String,
    pub quota: Option<BdcQuota>,
}

pub struct UtcCatalogResponse {
    pub server_utc: String,
    pub catalog_date: String,
    pub catalog_ubound: usize,
}

pub struct WorkResponse {
    pub info: String,
    pub text: Vec<u8>,
}

fn client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(45))
        .build()
        .map_err(|err| err.to_string())
}

fn rnd() -> String {
    Alphanumeric.sample_string(&mut rand::thread_rng(), 8)
}

pub fn cifra(input: &str) -> String {
    let len = input.chars().count();
    let mut v = vec![0u32; len + 1];

    for ch in input.chars() {
        v[0] += ch as u32;
    }
    v[0] %= 256;

    for (idx, ch) in input.chars().enumerate() {
        v[idx + 1] = v[idx] ^ (ch as u32);
    }

    let mut out = String::with_capacity((len + 1) * 2);
    for n in v {
        out.push_str(&format!("{:02X}", n & 0xFF));
    }
    out
}

fn is_protocol_error(text: &str) -> bool {
    text.trim_start().starts_with('!')
}

fn decode_server_text(bytes: &[u8]) -> String {
    if let Ok(text) = std::str::from_utf8(bytes) {
        text.to_string()
    } else {
        let (decoded, _, _) = WINDOWS_1252.decode(bytes);
        decoded.to_string()
    }
}

pub fn identify(username: &str, password: &str) -> Result<IdentifyResponse, String> {
    let query_plain = format!("{};{};{};*;{}", IDEN_SP, username, password, rnd());
    let query_enc = cifra(&query_plain);
    let url = format!("{BASE_URL}?{query_enc}");

    let bytes = client()?
        .get(&url)
        .send()
        .and_then(|resp| resp.error_for_status())
        .map_err(|err| err.to_string())?
        .bytes()
        .map_err(|err| err.to_string())?;
    let body = decode_server_text(&bytes);

    if is_protocol_error(&body) {
        return Err(body);
    }
    let parts: Vec<&str> = body.trim().split(';').collect();
    let nprov = parts
        .first()
        .copied()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Risposta identificazione non valida".to_string())?
        .to_string();
    let quota = parse_identify_quota(&parts);

    Ok(IdentifyResponse { nprov, quota })
}

fn parse_identify_quota(parts: &[&str]) -> Option<BdcQuota> {
    let remaining = parts.get(1)?.trim();
    if remaining.is_empty() {
        return None;
    }
    let monthly_total = parts
        .get(2)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or("60");
    Some(BdcQuota {
        remaining: remaining.to_string(),
        monthly_total: monthly_total.to_string(),
    })
}

pub fn parse_work_quota(info: &str) -> Option<BdcQuota> {
    let parts: Vec<&str> = info.trim().split(';').collect();
    let remaining = parts.get(1)?.trim();
    if remaining.is_empty() {
        return None;
    }
    let monthly_total = parts
        .get(4)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or("60");
    Some(BdcQuota {
        remaining: remaining.to_string(),
        monthly_total: monthly_total.to_string(),
    })
}

pub fn fetch_catalog_list(nprov: &str) -> Result<String, String> {
    let url = format!("{BASE_URL}?-ele;@{};{}", nprov, rnd());
    let bytes = client()?
        .get(&url)
        .send()
        .and_then(|resp| resp.error_for_status())
        .map_err(|err| err.to_string())?
        .bytes()
        .map_err(|err| err.to_string())?;
    let body = decode_server_text(&bytes);
    if is_protocol_error(&body) {
        return Err(body);
    }
    Ok(body)
}

pub fn fetch_latest_list(nprov: &str) -> Result<String, String> {
    let url = format!("{BASE_URL}?-ult;@{};{}", nprov, rnd());
    let bytes = client()?
        .get(&url)
        .send()
        .and_then(|resp| resp.error_for_status())
        .map_err(|err| err.to_string())?
        .bytes()
        .map_err(|err| err.to_string())?;
    let body = decode_server_text(&bytes);
    if is_protocol_error(&body) {
        return Err(body);
    }
    Ok(body)
}

pub fn fetch_catalog_utc(nprov: &str) -> Result<UtcCatalogResponse, String> {
    let url = format!("{BASE_URL}?-utc;@{};{}", nprov, rnd());
    let bytes = client()?
        .get(&url)
        .send()
        .and_then(|resp| resp.error_for_status())
        .map_err(|err| err.to_string())?
        .bytes()
        .map_err(|err| err.to_string())?;
    let body = decode_server_text(&bytes);
    if is_protocol_error(&body) {
        return Err(body);
    }

    let mut parts = body.split(';').map(str::trim);
    let server_utc = parts
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Risposta UTC non valida".to_string())?
        .to_string();
    let catalog_date = parts
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Data catalogo UTC non valida".to_string())?
        .to_string();
    let catalog_ubound = parts
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Numero opere UTC non valido".to_string())?
        .parse::<usize>()
        .map_err(|_| "Numero opere UTC non valido".to_string())?;

    Ok(UtcCatalogResponse {
        server_utc,
        catalog_date,
        catalog_ubound,
    })
}

pub fn parse_catalog_records(raw: &str) -> Vec<String> {
    raw.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('['))
        .map(ToOwned::to_owned)
        .collect()
}

pub fn download_work(
    username: &str,
    password: &str,
    index: &str,
    preview: bool,
) -> Result<WorkResponse, String> {
    let _power_awake = crate::power_awake::acquire("download");
    let utc = Utc::now().format("%Y-%m-%d %H.%M.%S").to_string();
    let sample = if preview { "+" } else { "" };
    let query_plain = format!(
        "{};{};{};{};{};{};150",
        IDEN_SP, username, password, index, utc, sample
    );
    let query_enc = cifra(&query_plain);
    let url = format!("{BASE_URL}?{query_enc}");

    let bytes = client()?
        .get(&url)
        .send()
        .and_then(|resp| resp.error_for_status())
        .map_err(|err| err.to_string())?
        .bytes()
        .map_err(|err| err.to_string())?
        .to_vec();

    if bytes.starts_with(b"!") {
        let text = decode_server_text(&bytes);
        if is_protocol_error(&text) {
            return Err(text);
        }
    }

    if let Some(pos) = bytes.iter().position(|b| *b == 26u8) {
        let info = decode_server_text(&bytes[..pos]);
        let text = bytes[pos + 1..].to_vec();
        return Ok(WorkResponse { info, text });
    }

    Ok(WorkResponse {
        info: String::new(),
        text: bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::{UtcCatalogResponse, decode_server_text};

    #[test]
    fn decode_server_text_preserves_cp1252_accents() {
        let bytes = b"libro Perch\xe9 NO.txt";
        assert_eq!(decode_server_text(bytes), "libro Perché NO.txt");
    }

    #[test]
    fn utc_catalog_response_fields_are_stored() {
        let response = UtcCatalogResponse {
            server_utc: "2026-03-15 12.00.00".to_string(),
            catalog_date: "2026-03-15".to_string(),
            catalog_ubound: 42,
        };
        assert_eq!(response.server_utc, "2026-03-15 12.00.00");
        assert_eq!(response.catalog_date, "2026-03-15");
        assert_eq!(response.catalog_ubound, 42);
    }
}
