use std::io::{Error, ErrorKind};
use std::path::PathBuf;

use anyhow::Result;
use colored::Colorize;
use starknet::signers::SigningKey;
use starknet_crypto::FieldElement;

pub fn new(password: Option<String>, force: bool, file: PathBuf) -> Result<()> {
    if file.exists() && !force {
        anyhow::bail!("keystore file already exists");
    }

    let password = get_password(password, true)?;

    let key = SigningKey::from_random();
    key.save_as_keystore(&file, &password)?;

    println!("Created new encrypted keystore file: {}", std::fs::canonicalize(file)?.display());
    println!("Public key: {}", format!("{:#064x}", key.verifying_key().scalar()).bright_yellow());

    Ok(())
}

pub fn from_key(
    force: bool,
    private_key: Option<String>,
    password: Option<String>,
    file: PathBuf,
) -> Result<()> {
    if file.exists() && !force {
        anyhow::bail!("keystore file already exists");
    }

    let private_key = if let Some(private_key) = private_key {
        private_key
    } else {
        rpassword::prompt_password("Enter private key: ")?
    };
    let private_key = FieldElement::from_hex_be(private_key.trim())?;

    let password = get_password(password, false)?;

    let key = SigningKey::from_secret_scalar(private_key);
    key.save_as_keystore(&file, &password)?;

    println!("Created new encrypted keystore file: {}", std::fs::canonicalize(file)?.display());
    println!("Public key: {:#064x}", key.verifying_key().scalar());

    Ok(())
}

pub fn inspect(password: Option<String>, raw: bool, file: PathBuf) -> Result<()> {
    if !file.exists() {
        anyhow::bail!("keystore file not found");
    }

    let password = get_password(password, false)?;

    let key = SigningKey::from_keystore(file, &password)?;

    if raw {
        println!("{:#064x}", key.verifying_key().scalar());
    } else {
        println!("Public key: {:#064x}", key.verifying_key().scalar());
    }

    Ok(())
}

pub fn inspect_private(password: Option<String>, raw: bool, file: PathBuf) -> Result<()> {
    if !file.exists() {
        anyhow::bail!("keystore file not found");
    }

    let password = get_password(password, false)?;

    let key = SigningKey::from_keystore(file, &password)?;

    if raw {
        println!("{:#064x}", key.secret_scalar());
    } else {
        println!("Private key: {:#064x}", key.secret_scalar());
    }

    Ok(())
}

fn get_password(password: Option<String>, retry: bool) -> std::io::Result<String> {
    if let Some(password) = password {
        Ok(password)
    } else {
        let password = rpassword::prompt_password("Enter password: ");

        if retry {
            let confirmpassword = rpassword::prompt_password("Confirm password: ");

            if password.clone()? != confirmpassword? {
                return Err(Error::new(ErrorKind::InvalidData, "Passwords do not match"));
            }
            return Ok(password?);
        };

        return Ok(password?);
    }
}
