pub mod credential {
    use account_sdk::wasm_webauthn::CredentialID;
    use base64::engine::general_purpose;
    use base64::{DecodeError, Engine};

    pub fn from_base64(base64: &str) -> Result<CredentialID, DecodeError> {
        let bytes = general_purpose::URL_SAFE_NO_PAD.decode(base64)?;
        Ok(CredentialID(bytes))
    }
}

pub mod cose_key {
    use anyhow::Result;
    use base64::engine::general_purpose;
    use base64::Engine;
    use coset::{CborSerializable, CoseKey};

    pub fn from_base64(base64: &str) -> Result<CoseKey> {
        let bytes = general_purpose::URL_SAFE_NO_PAD.decode(base64)?;
        Ok(CoseKey::from_slice(&bytes)?)
    }
}
