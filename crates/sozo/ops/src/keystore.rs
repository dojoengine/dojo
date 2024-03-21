use std::io::Read;
use std::path::PathBuf;

use anyhow::Result;
use colored::Colorize;
use starknet::signers::SigningKey;
use starknet_crypto::FieldElement;

const RAW_PASSWORD_WARNING: &str = "WARNING: setting passwords via --password is generally \
                                    considered insecure, as they will be stored in your shell \
                                    history or other log files.";

pub fn new(password: Option<String>, force: bool, file: PathBuf) -> Result<()> {
    if password.is_some() {
        eprintln!("{}", RAW_PASSWORD_WARNING.bright_magenta());
    }

    if file.exists() && !force {
        anyhow::bail!("keystore file already exists");
    }

    let password = get_password(password)?;

    let key = SigningKey::from_random();
    key.save_as_keystore(&file, &password)?;

    println!("Created new encrypted keystore file: {}", std::fs::canonicalize(file)?.display());
    println!("Public key: {}", format!("{:#064x}", key.verifying_key().scalar()).bright_yellow());

    Ok(())
}

pub fn from_key(
    force: bool,
    private_key_stdin: bool,
    password: Option<String>,
    file: PathBuf,
) -> Result<()> {
    if password.is_some() {
        eprintln!("{}", RAW_PASSWORD_WARNING.bright_magenta());
    }

    if file.exists() && !force {
        anyhow::bail!("keystore file already exists");
    }

    let private_key = if private_key_stdin {
        let mut buffer = String::new();
        std::io::stdin().read_to_string(&mut buffer)?;

        buffer
    } else {
        rpassword::prompt_password("Enter private key: ")?
    };
    let private_key = FieldElement::from_hex_be(private_key.trim())?;

    let password = get_password(password)?;

    let key = SigningKey::from_secret_scalar(private_key);
    key.save_as_keystore(&file, &password)?;

    println!("Created new encrypted keystore file: {}", std::fs::canonicalize(file)?.display());
    println!("Public key: {:#064x}", key.verifying_key().scalar());

    Ok(())
}

pub fn inspect(password: Option<String>, raw: bool, file: PathBuf) -> Result<()> {
    if password.is_some() {
        eprintln!("{}", RAW_PASSWORD_WARNING.bright_magenta());
    }

    if !file.exists() {
        anyhow::bail!("keystore file not found");
    }

    let password = get_password(password)?;

    let key = SigningKey::from_keystore(file, &password)?;

    if raw {
        println!("{:#064x}", key.verifying_key().scalar());
    } else {
        println!("Public key: {:#064x}", key.verifying_key().scalar());
    }

    Ok(())
}

pub fn inspect_private(password: Option<String>, raw: bool, file: PathBuf) -> Result<()> {
    if password.is_some() {
        eprintln!("{}", RAW_PASSWORD_WARNING.bright_magenta());
    }

    if !file.exists() {
        anyhow::bail!("keystore file not found");
    }

    let password = get_password(password)?;

    let key = SigningKey::from_keystore(file, &password)?;

    if raw {
        println!("{:#064x}", key.secret_scalar());
    } else {
        println!("Private key: {:#064x}", key.secret_scalar());
    }

    Ok(())
}

fn get_password(password: Option<String>) -> std::io::Result<String> {
    if let Some(password) = password {
        Ok(password)
    } else {
        rpassword::prompt_password("Enter password: ")
    }
}
