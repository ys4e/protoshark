use base64::Engine;
use base64::engine::general_purpose::STANDARD;

/// Decodes a standard Base64 string into a byte array.
/// 
/// # Example
/// 
/// ```rust,no_run
/// let data = base64_decode("SGVsbG8gV29ybGQ=");
/// assert_eq!(data.len(), 11);
/// ```
pub fn base64_decode<S: AsRef<str>>(data: S) -> Vec<u8> {
    STANDARD.decode(data.as_ref()).unwrap()
}

/// Encodes a byte array into a standard Base64 string.
/// 
/// # Example
/// 
/// ```rust,no_run
/// let data = &[0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x20, 0x57, 0x6f, 0x72, 0x6c, 0x64]; 
/// assert_eq!(base64_encode(data), "SGVsbG8gV29ybGQ=");
/// ```
pub fn base64_encode(data: &[u8]) -> String {
    STANDARD.encode(data)
}
