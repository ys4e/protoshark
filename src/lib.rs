pub(crate) mod utils;
pub mod bytes;
pub mod varint;

use std::{collections::BTreeMap, error::Error};

use paste::paste;
use serde::{Deserialize, Serialize};

// Re-export all `bytes` items.
pub use crate::bytes::*;

// Re-export all `varint` items.
pub use crate::varint::*;

type DecodeError = Box<dyn Error>;
pub type SerializedMessage = BTreeMap<u32, Value>;

/// Decodes a protobuf-encoded message.
///
/// `bytes`: A slice of bytes representing the protobuf-encoded message.
///
/// Returns a HashMap of field numbers to values.
pub fn decode(bytes: &[u8]) -> Result<SerializedMessage, DecodeError> {
    let bytes_len = bytes.len();

    let mut message: SerializedMessage = BTreeMap::new();
    let mut index = 0usize;

    while index < bytes.len() {
        let varint = VarInt::raw_at(bytes, index);
        let Ok(header) = Header::decode(&varint) else {
            return Err("Invalid wire type specified".into());
        };

        index += varint.len();

        match header.wire_type {
            WireType::VarInt => {
                let (varint, len) = VarInt::decode_at(bytes, index);
                index += len;

                message.insert(header.field_number, Value::VarInt(varint));
            }
            WireType::Fixed64 => {
                if bytes_len < index || bytes_len < index + 8 {
                    return Err("Invalid message; not enough bytes for a fixed64 field.".into());
                }

                let bytes: [u8; 8] = bytes[index..index + 8].try_into()?;
                index += 8;

                let value = f64::from_le_bytes(bytes);
                message.insert(header.field_number, Value::Double(value));
            }
            WireType::LengthDelimited => {
                let (data_len, varint_len) = VarInt::decode_at(bytes, index);
                index += varint_len;

                if bytes_len < index || bytes_len < index + data_len.as_i32() as usize {
                    return Err("Invalid message; not enough bytes for a length-delimited field.".into());
                }

                let bytes = &bytes[index..index + data_len.as_i32() as usize];
                index += data_len.as_i32() as usize;

                let data = decode(bytes);
                let string = std::str::from_utf8(bytes);

                if data.is_err() && string.is_err() {
                    message.insert(header.field_number, Value::Bytes(bytes.to_vec()));
                } else {
                    if let Ok(string) = string {
                        message.insert(header.field_number, Value::String(string.to_string()));
                    }
                    if let Ok(data) = data {
                        message.insert(header.field_number, Value::Message(data));
                    }
                }
            }
            WireType::StartGroup => {
                return Err("Start group wire type is not supported.".into());
            }
            WireType::EndGroup => {
                return Err("End group wire type is not supported.".into());
            }
            WireType::Fixed32 => {
                if bytes_len < index || bytes_len < index + 4 {
                    return Err("Invalid message; not enough bytes for a fixed32 field.".into());
                }

                let bytes: [u8; 4] = bytes[index..index + 4].try_into()?;
                index += 4;

                let value = f32::from_le_bytes(bytes);
                message.insert(header.field_number, Value::Float(value));
            }
        }
    }

    Ok(message)
}

struct Header {
    field_number: u32,
    wire_type: WireType
}

impl Header {
    /// Creates a new protobuf message header.
    pub fn new(field_number: u32, wire_type: WireType) -> Self {
        Self { field_number, wire_type }
    }

    /// Decodes a protobuf header.
    /// bytes: A slice of bytes representing the header.
    pub fn decode(bytes: &[u8]) -> Result<Self, ()> {
        let varint = VarInt::decode(bytes);
        let int = varint.as_u32().ok_or(())?;

        Ok(Self {
            field_number: int >> 3,
            wire_type: WireType::try_from(0b0000_0111 & int as u8)?
        })
    }

    /// Converts the header into a slice of bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![];
        self.encode(&mut bytes);
        bytes
    }

    /// Encodes the header into a slice of bytes.
    pub fn encode(&self, bytes: &mut Vec<u8>) {
        let wire_type: u32 = self.wire_type.into();
        let integer = (self.field_number << 3) | wire_type;

        bytes.append(&mut VarInt::encode(integer as i32));
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(u8)]
enum WireType {
    VarInt,
    Fixed64,
    LengthDelimited,
    StartGroup, /* These are deprecated. */
    EndGroup, /* These are deprecated. */
    Fixed32
}

impl TryFrom<u8> for WireType {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(WireType::VarInt),
            1 => Ok(WireType::Fixed64),
            2 => Ok(WireType::LengthDelimited),
            3 => Ok(WireType::StartGroup),
            4 => Ok(WireType::EndGroup),
            5 => Ok(WireType::Fixed32),
            _ => Err(())
        }
    }
}

impl Into<u32> for WireType {
    fn into(self) -> u32 {
        match self {
            WireType::VarInt => 0,
            WireType::Fixed64 => 1,
            WireType::LengthDelimited => 2,
            WireType::StartGroup => 3,
            WireType::EndGroup => 4,
            WireType::Fixed32 => 5
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Number {
    Integer(i32),
    Long(i64),
    UnsignedInteger(u32),
    UnsignedLong(u64)
}

impl Number {
    /// Determines which value the variable integer is closest to.
    pub fn closest(var_int: VarInt) -> Self {
        let mut i64: Option<i64> = None;
        let mut u32: Option<u32> = None;
        let mut u64: Option<u64> = None;

        // Always serialize i32
        let i32 = var_int.as_i32();

        // Serialize i64 if there are enough bytes (at least 8 bytes)
        if var_int.length() >= 8 {
            i64 = Some(var_int.as_i64());

            // Check if the i64 is the same as the i32
            if i64.unwrap() == i32 as i64 {
                i64 = None;
            }
        }

        // Serialize u32 if the value is non-negative
        if let Some(u32_val) = var_int.as_u32() {
            // If the u32 is the same as the i32, don't serialize it
            if i32 < 0 || i32 as u32 != u32_val {
                u32 = Some(u32_val);

                // Serialize u64 if there are enough bytes (at least 8 bytes) and the value is non-negative
                if var_int.length() >= 8 {
                    if let Some(u64_val) = var_int.as_u64() {
                        if u64_val != u32_val as u64 {
                            u64 = Some(u64_val);
                        }
                    }
                }
            }
        }

        if i64.is_none() && u32.is_none() && u64.is_none() {
            Number::Integer(i32)
        } else {
            if let Some(i64) = i64 {
                Number::Long(i64)
            } else if let Some(u32) = u32 {
                Number::UnsignedInteger(u32)
            } else if let Some(u64) = u64 {
                Number::UnsignedLong(u64)
            } else {
                Number::Integer(i32)
            }
        }
    }
}

macro_rules! value_conversion {
    ($($t:ty => $v:ident; $name:ident),*) => {
        $(
            impl From<$t> for Value {
                fn from(value: $t) -> Self {
                    Value::$v(value)
                }
            }

            impl Into<$t> for Value {
                fn into(self) -> $t {
                    match self {
                        Value::$v(value) => value,
                        _ => panic!("Invalid conversion.")
                    }
                }
            }

            paste! {
                impl Value {
                    pub fn [<as_ $name:lower>](&self) -> Option<$t> {
                        match self {
                            Value::$v(value) => Some(value.clone()),
                            _ => None
                        }
                    }
                }
            }
        )*
    };
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    VarInt(VarInt),
    Float(f32),
    Double(f64),
    String(String),
    #[serde(with = "base64")]
    Bytes(Vec<u8>),
    Message(SerializedMessage)
}

value_conversion!(
    VarInt => VarInt; varint,
    f32 => Float; float,
    f64 => Double; double,
    String => String; string,
    Vec<u8> => Bytes; bytes,
    SerializedMessage => Message; message
);

// Special conversions.

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::VarInt(if value { 1 } else { 0 }.into())
    }
}

impl Into<bool> for Value {
    fn into(self) -> bool {
        match self {
            Value::VarInt(value) => match value.as_i32() {
                0 => false,
                1 => true,
                _ => panic!("Invalid conversion.")
            },
            _ => panic!("Invalid conversion.")
        }
    }
}

impl Value {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::VarInt(value) => match value.as_i32() {
                0 => Some(false),
                1 => Some(true),
                _ => None
            },
            _ => None
        }
    }

    pub fn as_i32(&self) -> Option<i32> {
        match self {
            Value::VarInt(value) => Some(value.as_i32()),
            _ => None
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::VarInt(value) => Some(value.as_i64()),
            _ => None
        }
    }

    pub fn as_u32(&self) -> Option<u32> {
        match self {
            Value::VarInt(value) => value.as_u32(),
            _ => None
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Value::VarInt(value) => value.as_u64(),
            _ => None
        }
    }
}

mod base64 {
    use crate::utils;
    use serde::{Serialize, Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &Vec<u8>, s: S) -> Result<S::Ok, S::Error> {
        let base64 = utils::base64_encode(v);
        String::serialize(&base64, s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let base64 = String::deserialize(d)?;
        Ok(utils::base64_decode(base64))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_all() {
        let message = utils::base64_decode(
            "CMr7/f///////wEQgbCkvIv9////ARiaiigg/8/bw/QCLcP1SEAxswxxHH+ELkE4AUINSGVsbG8sIFdvcmxkIUogy7Z2rm0bzr4uZoGQPV2M+i52+c6kZtCFIKs/il2DQXdQAlovIgh5ZWFoeWVhaHog+RnnJSsU6kdRW/n67wdtWq59l0BbgApj5M6jlnpwZKDIOAA="
        );
        let decoded = decode(&message).expect("Failed to decode the message.");

        let json = serde_json::to_string(&decoded).unwrap();
        assert_eq!(json, r#"{"1":-33334,"2":[-1215752191,-99999999999],"3":656666,"4":1215752191,"5":3.14,"6":999999.55555,"7":1,"8":"Hello, World!","9":"y7Z2rm0bzr4uZoGQPV2M+i52+c6kZtCFIKs/il2DQXc=","10":2,"11":{"4":"yeahyeah","15":"+RnnJSsU6kdRW/n67wdtWq59l0BbgApj5M6jlnpwZKA=","905":0}}"#);
    }
}
