use mistake_trainer_next_lib::infrastructure::assets::{
    AssetCryptoError, decrypt_asset, encrypt_asset, plaintext_sha256,
};

#[test]
fn encrypted_asset_round_trips_without_exposing_plaintext() {
    let key = [7_u8; 32];
    let plaintext = b"private mistake image bytes";
    let encrypted = encrypt_asset(plaintext, &key).expect("encrypt asset");

    assert!(
        !encrypted
            .windows(plaintext.len())
            .any(|window| window == plaintext)
    );
    assert_eq!(
        decrypt_asset(&encrypted, &key).expect("decrypt asset"),
        plaintext
    );
}

#[test]
fn duplicate_plaintext_has_one_hash_but_fresh_ciphertext() {
    let key = [11_u8; 32];
    let plaintext = b"same image";

    let first = encrypt_asset(plaintext, &key).expect("first encryption");
    let second = encrypt_asset(plaintext, &key).expect("second encryption");

    assert_eq!(plaintext_sha256(plaintext), plaintext_sha256(plaintext));
    assert_ne!(first, second);
}

#[test]
fn tampering_is_rejected() {
    let key = [19_u8; 32];
    let mut encrypted = encrypt_asset(b"answer image", &key).expect("encrypt asset");
    let last = encrypted.len() - 1;
    encrypted[last] ^= 0x01;

    assert!(matches!(
        decrypt_asset(&encrypted, &key),
        Err(AssetCryptoError::Authentication)
    ));
}

#[test]
fn invalid_blob_header_is_rejected_before_decryption() {
    let key = [23_u8; 32];
    assert!(matches!(
        decrypt_asset(b"not-an-asset", &key),
        Err(AssetCryptoError::InvalidFormat)
    ));
}
