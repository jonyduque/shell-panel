/// PowerShell integration script, embedded in the binary and passed with `-EncodedCommand`:
/// no file is written and the user's execution policy is left alone.
pub const SCRIPT: &str = include_str!("../../assets/shellIntegration.ps1");

/// Standard base64 with padding.
pub fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = (chunk[0] as u32) << 16
            | (*chunk.get(1).unwrap_or(&0) as u32) << 8
            | *chunk.get(2).unwrap_or(&0) as u32;
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// A secret that tags every message the integration script sends. Programs run in the session
/// print into the same stream, but their output cannot know this value, so it cannot forge a
/// ReadLine marker or a completion report. (Code the user runs in the session is not the threat:
/// it can type into the console anyway.)
pub fn new_session_token() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    // RandomState::new() reuses one per-thread 128-bit key from the OS random source and
    // increments it, so the two halves are SipHash outputs under related keys, not two
    // independent draws. The secret's strength is that 128-bit OS-random key.
    (0..2u8)
        .map(|i| {
            let mut hasher = RandomState::new().build_hasher();
            hasher.write_u8(i);
            format!("{:016x}", hasher.finish())
        })
        .collect()
}

/// The integration script for one session: [`SCRIPT`] with the token written into `__SP-Send`
/// as a literal, so no variable of the session holds it.
pub fn script(token: &str) -> String {
    assert!(
        !token.is_empty() && token.chars().all(|c| c.is_ascii_hexdigit()),
        "the session token must be non-empty ASCII hex, got {} characters",
        token.chars().count()
    );
    SCRIPT.replace("__SP_TOKEN__", token)
}

/// [`script`] as `-EncodedCommand` expects it: base64 of its UTF-16LE bytes.
pub fn encoded_command(token: &str) -> String {
    let utf16: Vec<u8> = script(token)
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    base64_encode(&utf16)
}
