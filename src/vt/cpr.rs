/// Checks if bytes contain CPR (Cursor Position Report) query `\x1b[6n` or `\x1b[?6n`.
pub fn has_cpr_query(data: &[u8]) -> bool {
    data.windows(4).any(|w| w == b"\x1b[6n") || data.windows(5).any(|w| w == b"\x1b[?6n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_cpr_query_standard() {
        assert!(has_cpr_query(b"\x1b[6n"));
        assert!(has_cpr_query(b"\x1b[?6n"));
        assert!(has_cpr_query(b"some prefix \x1b[6n and suffix"));
        assert!(has_cpr_query(b"prefix \x1b[?6n suffix"));
        assert!(!has_cpr_query(b"normal text"));
        assert!(!has_cpr_query(b""));
        assert!(!has_cpr_query(b"\x1b[5n"));
    }
}
