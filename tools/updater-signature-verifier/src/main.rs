use std::{
    env,
    fs::{self, File},
    io::Read,
    path::Path,
};

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use minisign_verify::{PublicKey, Signature};

const BUFFER_SIZE: usize = 1024 * 1024;

fn main() -> Result<()> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        bail!("usage: updater-signature-verifier <artifact> <signature> <tauri-config>");
    }

    verify(
        Path::new(&args[1]),
        Path::new(&args[2]),
        Path::new(&args[3]),
    )?;
    println!("Updater signature verified against the baked application public key.");
    Ok(())
}

fn verify(artifact_path: &Path, signature_path: &Path, config_path: &Path) -> Result<()> {
    let config: serde_json::Value = serde_json::from_slice(
        &fs::read(config_path).context("failed to read the Tauri configuration")?,
    )
    .context("failed to parse the Tauri configuration")?;

    let encoded_public_key = config
        .pointer("/plugins/updater/pubkey")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .context("the Tauri updater public key is missing")?;
    let public_key_text = decode_base64_text(encoded_public_key, "updater public key")?;
    let public_key = PublicKey::decode(&public_key_text)
        .context("the baked updater public key is not valid minisign data")?;

    // Tauri stores the complete minisign signature document as a base64 string
    // in both the `.sig` asset and `latest.json`.
    let encoded_signature =
        fs::read_to_string(signature_path).context("failed to read the updater signature")?;
    let signature_text = decode_base64_text(encoded_signature.trim(), "updater signature")?;
    let signature = Signature::decode(&signature_text)
        .context("the updater signature is not valid minisign data")?;

    let mut artifact = File::open(artifact_path).context("failed to open the updater artifact")?;
    if artifact
        .metadata()
        .context("failed to inspect the updater artifact")?
        .len()
        == 0
    {
        bail!("the updater artifact is empty");
    }

    // `minisign-verify` is the same verifier used by tauri-plugin-updater. Its
    // streaming API validates the key id, prehashed Ed25519 signature and
    // trusted-comment global signature without loading the installer into RAM.
    let mut verifier = public_key
        .verify_stream(&signature)
        .context("the updater signature is not compatible with streaming verification")?;
    let mut buffer = vec![0_u8; BUFFER_SIZE];
    loop {
        let count = artifact
            .read(&mut buffer)
            .context("failed while reading the updater artifact")?;
        if count == 0 {
            break;
        }
        verifier.update(&buffer[..count]);
    }
    verifier
        .finalize()
        .context("updater signature verification failed")
}

fn decode_base64_text(value: &str, label: &str) -> Result<String> {
    let decoded = STANDARD
        .decode(value)
        .with_context(|| format!("{label} is not valid base64"))?;
    String::from_utf8(decoded).with_context(|| format!("{label} is not valid UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::decode_base64_text;

    #[test]
    fn rejects_non_base64_input() {
        assert!(decode_base64_text("not base64!", "test value").is_err());
    }
}
