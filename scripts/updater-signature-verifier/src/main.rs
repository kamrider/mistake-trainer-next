use base64::{Engine as _, engine::general_purpose::STANDARD};
use minisign_verify::{PublicKey, Signature};
use std::{
    env,
    error::Error,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

fn decode_tauri_public_key(encoded: &str) -> Result<PublicKey, Box<dyn Error>> {
    let decoded = STANDARD.decode(encoded.trim())?;
    let minisign_text = String::from_utf8(decoded)?;
    Ok(PublicKey::decode(&minisign_text)?)
}

fn decode_tauri_signature(path: &Path) -> Result<Signature, Box<dyn Error>> {
    let encoded = fs::read_to_string(path)?;
    let decoded = STANDARD.decode(encoded.trim())?;
    let minisign_text = String::from_utf8(decoded)?;
    Ok(Signature::decode(&minisign_text)?)
}

fn verify_payload(
    payload_path: &Path,
    signature_path: &Path,
    encoded_public_key: &str,
) -> Result<(), Box<dyn Error>> {
    let public_key = decode_tauri_public_key(encoded_public_key)?;
    let signature = decode_tauri_signature(signature_path)?;
    let mut verifier = public_key.verify_stream(&signature)?;
    let mut payload = File::open(payload_path)?;
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let bytes_read = payload.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        verifier.update(&buffer[..bytes_read]);
    }

    verifier.finalize()?;
    Ok(())
}

fn required_path_argument(
    arguments: &mut impl Iterator<Item = String>,
    name: &str,
) -> Result<PathBuf, Box<dyn Error>> {
    arguments
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| format!("missing {name} argument").into())
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut arguments = env::args().skip(1);
    let payload_path = required_path_argument(&mut arguments, "payload path")?;
    let signature_path = required_path_argument(&mut arguments, "signature path")?;
    if arguments.next().is_some() {
        return Err("unexpected extra argument".into());
    }

    let encoded_public_key = env::var("WINDOWS_UPDATER_PUBLIC_KEY")?;
    verify_payload(&payload_path, &signature_path, &encoded_public_key)?;
    println!("Tauri updater signature verified.");
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Tauri updater signature verification failed: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    const PUBLIC_KEY: &str = "untrusted comment: minisign public key E7620F1842B4E81F\nRWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3";
    const WRONG_PUBLIC_KEY: &str = "untrusted comment: minisign public key E7620F1842B4E81F\nRWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO2";
    const SIGNATURE: &str = "untrusted comment: signature from minisign secret key\nRUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=\ntrusted comment: timestamp:1556193335\tfile:test\ny/rUw2y8/hOUYjZU71eHp/Wo1KZ40fGy2VJEDl34XMJM+TX48Ss/17u3IvIfbVR1FkZZSNCisQbuQY+bHwhEBg==";
    static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(1);

    struct Fixture {
        root: PathBuf,
        payload: PathBuf,
        signature: PathBuf,
    }

    impl Fixture {
        fn new(payload: &[u8], signature: &str) -> Self {
            let id = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
            let root = env::temp_dir().join(format!(
                "mistake-trainer-updater-verifier-{}-{id}",
                std::process::id()
            ));
            fs::create_dir(&root).expect("create fixture directory");
            let payload_path = root.join("payload.bin");
            let signature_path = root.join("payload.bin.sig");
            fs::write(&payload_path, payload).expect("write fixture payload");
            fs::write(&signature_path, STANDARD.encode(signature))
                .expect("write fixture signature");
            Self {
                root,
                payload: payload_path,
                signature: signature_path,
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn encoded_public_key(value: &str) -> String {
        STANDARD.encode(value)
    }

    #[test]
    fn accepts_valid_payload_signature_and_public_key() {
        let fixture = Fixture::new(b"test", SIGNATURE);
        verify_payload(
            &fixture.payload,
            &fixture.signature,
            &encoded_public_key(PUBLIC_KEY),
        )
        .expect("valid signature should verify");
    }

    #[test]
    fn rejects_tampered_payload() {
        let fixture = Fixture::new(b"Test", SIGNATURE);
        assert!(
            verify_payload(
                &fixture.payload,
                &fixture.signature,
                &encoded_public_key(PUBLIC_KEY),
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_tampered_signature() {
        let tampered = SIGNATURE.replacen("RUQf6L", "RUQf7L", 1);
        let fixture = Fixture::new(b"test", &tampered);
        assert!(
            verify_payload(
                &fixture.payload,
                &fixture.signature,
                &encoded_public_key(PUBLIC_KEY),
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_wrong_public_key() {
        let fixture = Fixture::new(b"test", SIGNATURE);
        assert!(
            verify_payload(
                &fixture.payload,
                &fixture.signature,
                &encoded_public_key(WRONG_PUBLIC_KEY),
            )
            .is_err()
        );
    }
}
