//! Snapshot v2 support: model manifest, canonical serialization, host-side
//! assertions (guide items 4 & 7; snapshot assembly lives in `SchemeLogic`).

use serde_json::Value;
use std::path::Path;

/// Model package manifest (`model.json` in the model directory, item 7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelManifest {
    pub name: String,
    pub state_schema: u64,
    pub default_seed: u64,
    pub assertions: Option<String>,
}

impl Default for ModelManifest {
    /// Defaults used when a model directory has no `model.json` (legacy
    /// models such as the bundled `scripts/` must keep working).
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            state_schema: 1,
            default_seed: 0,
            assertions: None,
        }
    }
}

impl ModelManifest {
    pub fn load(scripts_dir: &Path) -> Self {
        let path = scripts_dir.join("model.json");
        let Ok(content) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        let Ok(v) = serde_json::from_str::<Value>(&content) else {
            tracing::warn!(path = %path.display(), "model.json is not valid JSON; using defaults");
            return Self::default();
        };
        Self {
            name: v
                .get("name")
                .and_then(|x| x.as_str())
                .unwrap_or("default")
                .to_string(),
            state_schema: v.get("state_schema").and_then(|x| x.as_u64()).unwrap_or(1),
            default_seed: v.get("default_seed").and_then(|x| x.as_u64()).unwrap_or(0),
            assertions: v
                .get("assertions")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string()),
        }
    }
}

/// Recursively rebuild a JSON value with object keys in sorted order.
/// (`serde_json` without `preserve_order` already stores maps sorted, but
/// values arriving from HashMap-backed props make an explicit canonical
/// pass the safer contract for hashing.)
pub fn canonicalize(v: &Value) -> Value {
    match v {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut out = serde_json::Map::new();
            for k in keys {
                out.insert(k.clone(), canonicalize(&map[k]));
            }
            Value::Object(out)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(canonicalize).collect()),
        other => other.clone(),
    }
}

/// SHA-256 hex digest of the canonical serialization of `v`.
pub fn hash_json(v: &Value) -> String {
    let canon = canonicalize(v);
    let text = serde_json::to_string(&canon).unwrap_or_default();
    sha256_hex(text.as_bytes())
}

/// SHA-256 hex digest of a model's script files (sorted by file name,
/// name + content), used as `scripts_hash` in snapshots and epoch reports.
pub fn scripts_hash(scripts_dir: &Path) -> String {
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(scripts_dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok().map(|x| x.path()))
                .filter(|p| p.extension().is_some_and(|x| x == "scm"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    let mut buf = Vec::new();
    for f in files {
        buf.extend_from_slice(f.file_name().unwrap_or_default().to_string_lossy().as_bytes());
        buf.push(0);
        if let Ok(content) = std::fs::read(&f) {
            buf.extend_from_slice(&content);
        }
        buf.push(0);
    }
    sha256_hex(&buf)
}

/// Minimal SHA-256 implementation (FIPS 180-4). The guide suggests the
/// `sha2` crate; this local implementation keeps the build self-contained
/// and is verified against the standard "abc" / empty test vectors below.
pub fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([chunk[4 * i], chunk[4 * i + 1], chunk[4 * i + 2], chunk[4 * i + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    let mut out = String::with_capacity(64);
    for word in h {
        out.push_str(&format!("{:08x}", word));
    }
    out
}

/// Navigate a dot-separated path (object keys / array indices).
pub fn navigate_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut curr = root;
    for seg in path.split('.') {
        if seg.is_empty() {
            continue;
        }
        match curr {
            Value::Object(map) => curr = map.get(seg)?,
            Value::Array(arr) => curr = arr.get(seg.parse::<usize>().ok()?)?,
            _ => return None,
        }
    }
    Some(curr)
}

fn json_num(v: &Value) -> Option<f64> {
    v.as_f64()
}

/// Evaluate host-side assertions against an exported state JSON.
/// Format per guide 4.4: `{path, op, expected}` with ops
/// `eq | ne | lt | lte | gt | gte | exists | absent`.
/// Returns one result object per assertion; never panics.
pub fn eval_host_assertions(state: &Value, assertions: &[Value]) -> Vec<Value> {
    let mut results = Vec::with_capacity(assertions.len());
    for a in assertions {
        let path = a.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let op = a.get("op").and_then(|v| v.as_str()).unwrap_or("eq");
        let expected = a.get("expected").cloned().unwrap_or(Value::Null);
        let actual_opt = navigate_path(state, path);
        let (passed, actual) = match op {
            "exists" => (actual_opt.is_some(), actual_opt.cloned().unwrap_or(Value::Null)),
            "absent" => (actual_opt.is_none(), actual_opt.cloned().unwrap_or(Value::Null)),
            "eq" => (
                actual_opt.is_some_and(|x| x == &expected),
                actual_opt.cloned().unwrap_or(Value::Null),
            ),
            "ne" => (
                actual_opt.is_some_and(|x| x != &expected),
                actual_opt.cloned().unwrap_or(Value::Null),
            ),
            "lt" | "lte" | "gt" | "gte" => {
                let cmp = match (actual_opt.and_then(json_num), json_num(&expected)) {
                    (Some(x), Some(y)) => x.partial_cmp(&y),
                    _ => None,
                };
                let ok = match (op, cmp) {
                    ("lt", Some(std::cmp::Ordering::Less)) => true,
                    ("lte", Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)) => true,
                    ("gt", Some(std::cmp::Ordering::Greater)) => true,
                    ("gte", Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)) => true,
                    _ => false,
                };
                (ok, actual_opt.cloned().unwrap_or(Value::Null))
            }
            _ => (false, actual_opt.cloned().unwrap_or(Value::Null)),
        };
        results.push(serde_json::json!({
            "path": path,
            "op": op,
            "expected": expected,
            "actual": actual,
            "passed": passed,
        }));
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn canonicalize_sorts_keys() {
        let v = serde_json::json!({"b": {"d": 1, "c": 2}, "a": 1});
        let c = canonicalize(&v);
        assert_eq!(serde_json::to_string(&c).unwrap(), "{\"a\":1,\"b\":{\"c\":2,\"d\":1}}");
        assert_eq!(hash_json(&v), hash_json(&c));
    }

    #[test]
    fn host_assertions_ops() {
        let state = serde_json::json!({"players": {"1": {"hp": 90}}, "tick_count": 200});
        let assertions = vec![
            serde_json::json!({"path": "players.1.hp", "op": "eq", "expected": 90}),
            serde_json::json!({"path": "players.1.hp", "op": "gte", "expected": 100}),
            serde_json::json!({"path": "tick_count", "op": "exists"}),
            serde_json::json!({"path": "nope", "op": "absent"}),
        ];
        let r = eval_host_assertions(&state, &assertions);
        assert_eq!(r[0]["passed"], true);
        assert_eq!(r[1]["passed"], false);
        assert_eq!(r[1]["actual"], 90);
        assert_eq!(r[2]["passed"], true);
        assert_eq!(r[3]["passed"], true);
    }
}
