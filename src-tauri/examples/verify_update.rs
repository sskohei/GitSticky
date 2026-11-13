use base64::{engine::general_purpose::STANDARD, Engine};
use minisign_verify::{PublicKey, Signature};
use std::{error::Error, fs, io};

// Verify the real updater artifact without launching the application or installer.
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err(
            io::Error::other("Usage: verify_update <installer> <signature> <version>").into(),
        );
    }
    let key = std::env::var("STICKYNOTE_UPDATER_PUBLIC_KEY")?;
    let key = String::from_utf8(STANDARD.decode(key.trim())?)?;
    let signature = fs::read_to_string(&args[1])?;
    let signature = String::from_utf8(STANDARD.decode(signature.trim())?)?;
    let key = PublicKey::decode(&key)?;
    let signature = Signature::decode(&signature)?;
    key.verify(&fs::read(&args[0])?, &signature, true)?;
    let signed_version = signature
        .trusted_comment()
        .split('\t')
        .find_map(|field| field.strip_prefix("version:"));
    if signed_version != Some(args[2].as_str()) {
        return Err(io::Error::other("Signature version does not match the release").into());
    }
    println!("Updater signature verified for v{}.", args[2]);
    Ok(())
}
