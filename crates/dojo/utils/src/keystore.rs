use anyhow::{anyhow, Result};

/// Prompts the user for a password if no password is provided and `no_wait` is not set.
/// The `no_wait` is used for non-interactive commands.
pub fn prompt_password_if_needed(maybe_password: Option<&str>, no_wait: bool) -> Result<String> {
    if let Some(password) = maybe_password {
        Ok(password.to_owned())
    } else if no_wait {
        Err(anyhow!("Could not find password. Please specify the password."))
    } else {
        Ok(rpassword::prompt_password("Enter the keystore password: ")?.to_owned())
    }
}
