use std::fmt::Write;

use sha2::{Digest, Sha256};

/// Stable content identity for immutable plaintext assets.
pub fn plaintext_sha256(plaintext: &[u8]) -> String {
    let digest = Sha256::digest(plaintext);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::plaintext_sha256;

    #[test]
    fn plaintext_hash_is_stable_lowercase_hex() {
        assert_eq!(
            plaintext_sha256(b"mistake-trainer"),
            "ba894a88d0c0d6c2058e55c6f8979f55eeb1ab19369450ad9f2decc51e0e80f8"
        );
    }
}
