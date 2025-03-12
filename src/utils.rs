use base64::Engine;
use base64::engine::general_purpose::STANDARD;

/// Decodes a standard Base64 string into a byte array.
pub fn base64_decode<S: AsRef<str>>(data: S) -> Vec<u8> {
    STANDARD.decode(data.as_ref()).unwrap()
}

/// Encodes a byte array into a standard Base64 string.
pub fn base64_encode(data: &[u8]) -> String {
    STANDARD.encode(data)
}
