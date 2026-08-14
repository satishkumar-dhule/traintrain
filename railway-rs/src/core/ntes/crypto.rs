//! NTES mobile API payload crypto.
//!
//! Protocol (reverse-engineered, verified against the live service):
//! `POST /crisns/AppServAnd` with JSON body `{"jsonIn": "<MD5_HEX>#<HEX>"}` where
//! - `<MD5_HEX>` = uppercase hex MD5 of `payload + SCKEY`
//! - `<HEX>`    = uppercase hex of the UTF-8 bytes of the base64 string produced
//!   by AES-128-CBC-encrypting `payload` (PKCS7 padding) with the fixed key/IV.
//!
//! Responses come back the same shape: `{"jsonIn": "<HASH>#<HEX>"}` which must be
//! hex-decoded, base64-decoded, AES-CBC-decrypted and un-padded.

use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use base64::Engine;
use md5::{Digest, Md5};

use super::super::error::AppError;

const KEY_BYTES: &[u8; 16] = b"8EA4DB2CC1EB3DC5";
const IV_BYTES: &[u8; 16] = b"7DC5EB3BB4DB6EA8";
const SCKEY: &str = "645fbc1e56e23365f2f3c204ae0899f6";
const BLOCK: usize = 16;

type Aes128CbcEnc = cbc::Encryptor<aes::Aes128>;
type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;

pub struct NtesCrypto;

impl NtesCrypto {
    /// `MD5(payload + sckey)` as uppercase hex.
    pub fn hash(payload: &str) -> String {
        let mut hasher = Md5::new();
        hasher.update(payload.as_bytes());
        hasher.update(SCKEY.as_bytes());
        let digest = hasher.finalize();
        hex::encode_upper(digest)
    }

    /// AES-128-CBC encrypt with the exact padding scheme the NTES server uses
    /// (verified against Node's `createCipheriv` + `final()` output):
    /// 1. manual PKCS7 pad to a block boundary, then
    /// 2. an extra full block of `0x10` (OpenSSL always pads, even when the
    ///    input is already block-aligned).
    ///
    /// Then `base64(bytes) -> utf8 string -> uppercase hex`.
    pub fn encrypt(payload: &str) -> String {
        let data = payload.as_bytes();
        let pad_len = BLOCK - (data.len() % BLOCK);
        let mut padded = vec![0u8; data.len() + pad_len + BLOCK];
        padded[..data.len()].copy_from_slice(data);
        padded[data.len()..data.len() + pad_len].fill(pad_len as u8);
        padded[data.len() + pad_len..].fill(BLOCK as u8);

        let buf_len = padded.len();
        let encrypted = Aes128CbcEnc::new(KEY_BYTES.into(), IV_BYTES.into())
            .encrypt_padded_mut::<aes::cipher::block_padding::NoPadding>(&mut padded, buf_len)
            .expect("padded buffer sized correctly")
            .to_vec();

        let b64 = base64::engine::general_purpose::STANDARD.encode(&encrypted);
        hex::encode_upper(b64.as_bytes())
    }

    /// Build the `"<HASH>#<ENC>"` request value for a plain payload.
    pub fn build(payload: &str) -> String {
        format!("{}#{}", Self::hash(payload), Self::encrypt(payload))
    }

    /// Decrypt a response value (`"<HASH>#<ENC>"` or bare `"<ENC>"`).
    pub fn decrypt(enc: &str) -> Result<String, AppError> {
        let s = match enc.split_once('#') {
            Some((_, rest)) => rest,
            None => enc,
        };
        if s.len() % 2 != 0 {
            return Err(AppError::internal("NTES: odd-length hex ciphertext"));
        }
        let b64bytes =
            hex::decode(s).map_err(|_| AppError::internal("NTES: invalid hex ciphertext"))?;
        let b64 = String::from_utf8(b64bytes)
            .map_err(|_| AppError::internal("NTES: ciphertext is not valid utf8"))?;
        let ct = base64::engine::general_purpose::STANDARD
            .decode(b64.trim())
            .map_err(|_| AppError::internal("NTES: invalid base64 ciphertext"))?;
        if ct.is_empty() || ct.len() % BLOCK != 0 {
            return Err(AppError::internal(
                "NTES: ciphertext length not a multiple of 16",
            ));
        }
        let mut buf = ct.clone();
        let dec = Aes128CbcDec::new(KEY_BYTES.into(), IV_BYTES.into())
            .decrypt_padded_mut::<aes::cipher::block_padding::Pkcs7>(&mut buf)
            .map_err(|_| AppError::internal("NTES: AES decrypt failed (bad padding)"))?;
        // Strip the trailing OpenSSL full-block pad (already removed by Pkcs7),
        // then the manual PKCS7-style pad the NTES protocol adds.
        let pad_len = dec.last().copied().unwrap_or(0) as usize;
        let len = dec.len().saturating_sub(pad_len);
        Ok(String::from_utf8_lossy(&dec[..len]).into_owned())
    }

    /// Decrypt a response and parse it as JSON.
    pub fn decode_json(enc: &str) -> Result<serde_json::Value, AppError> {
        let plain = Self::decrypt(enc)?;
        serde_json::from_str(&plain)
            .map_err(|_| AppError::internal("NTES: decrypted payload is not JSON"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAYLOAD: &str =
        "service=TrainRunningMob&subService=GetTrainSchedule&trainNo=12951&startDate=";

    #[test]
    fn golden_hash_matches_node_implementation() {
        assert_eq!(
            NtesCrypto::hash(PAYLOAD),
            "19CBAF6FE7CBDB479D0FC31FD28A9C5F"
        );
    }

    #[test]
    fn golden_encrypt_matches_node_implementation() {
        assert_eq!(
            NtesCrypto::encrypt(PAYLOAD),
            "6430384136355152636A2B7345435539465A72522B37364E7358716157656E6F756C52516F4A61695137766F626F4F7134597878445A62423061677A6841417453714E303959635A6A70384A6874477775437467323174717848366D796D2F595048644649727A4E43545573614A75623745714531596B4D3776775162673937"
        );
    }

    #[test]
    fn build_has_expected_shape() {
        let b = NtesCrypto::build(PAYLOAD);
        let (h, enc) = b.split_once('#').unwrap();
        assert_eq!(h, NtesCrypto::hash(PAYLOAD));
        assert_eq!(enc, NtesCrypto::encrypt(PAYLOAD));
    }

    #[test]
    fn decrypt_roundtrips_build() {
        let b = NtesCrypto::build(PAYLOAD);
        assert_eq!(NtesCrypto::decrypt(&b).unwrap(), PAYLOAD);
    }

    #[test]
    fn decrypt_accepts_bare_ciphertext() {
        let enc = NtesCrypto::encrypt(PAYLOAD);
        assert_eq!(NtesCrypto::decrypt(&enc).unwrap(), PAYLOAD);
    }

    #[test]
    fn decrypt_rejects_garbage() {
        assert!(NtesCrypto::decrypt("zz").is_err());
        assert!(NtesCrypto::decrypt("abcd1234").is_err());
    }
}
