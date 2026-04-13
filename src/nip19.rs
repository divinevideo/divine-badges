use bech32::{Bech32, Hrp};

use crate::error::AppError;

pub fn encode_npub(hex_pubkey: &str) -> Result<String, AppError> {
    let data = hex::decode(hex_pubkey)
        .map_err(|err| AppError::Config(format!("invalid hex pubkey: {err}")))?;
    if data.len() != 32 {
        return Err(AppError::Config(format!(
            "pubkey must be 32 bytes, got {}",
            data.len()
        )));
    }
    let hrp = Hrp::parse("npub").map_err(|err| AppError::Config(err.to_string()))?;
    bech32::encode::<Bech32>(hrp, &data).map_err(|err| AppError::Config(err.to_string()))
}
