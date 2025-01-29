mod utils;

use core::fmt;
use std::{collections::BTreeMap, error::Error};

use paste::paste;
use serde::{de::{SeqAccess, Visitor}, ser::SerializeSeq, Deserialize, Deserializer, Serialize, Serializer};

type DecodeError = Box<dyn Error>;
pub type SerializedMessage = BTreeMap<i32, Value>;

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
    field_number: i32,
    wire_type: WireType
}

impl Header {
    /// Decodes a protobuf header.
    /// bytes: A slice of bytes representing the header.
    pub fn decode(bytes: &[u8]) -> Result<Self, ()> {
        let varint = VarInt::decode(bytes);
        let int = varint.as_i32();

        Ok(Self {
            field_number: int >> 3,
            wire_type: WireType::try_from(0b0000_0111 & int as u8)?
        })
    }
}

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

#[derive(Clone, Debug)]
pub struct VarInt(Vec<u8>);

impl VarInt {
    /// Decodes a variable integer into a 32-bit unsigned integer.
    /// bytes: A slice of bytes representing the variable integer.
    pub fn decode(bytes: &[u8]) -> VarInt {
        let mut int_bytes = vec![];
        bytes.iter().for_each(|byte| {
            let byte = byte & 0b0111_1111;
            int_bytes.push(byte);
        });
        int_bytes.reverse();

        VarInt(int_bytes)
    }

    /// Encodes a 32-bit unsigned integer into a variable integer.
    /// value: The 32-bit unsigned integer to encode.
    pub fn encode(value: i32) -> Vec<u8> {
        let mut bytes = vec![];
        for i in 0..5 {
            let mut byte = (value >> (i * 7)) as u8;
            if i == 4 {
                byte &= 0b0001_1111;
            } else {
                byte |= 0b1000_0000;
            }
            bytes.push(byte);
        }

        bytes
    }

    /// Decodes a variable integer at a specific index.
    /// bytes: A slice of bytes representing the variable integer.
    /// index: The index to start reading the bytes from.
    pub fn decode_at(bytes: &[u8], index: usize) -> (VarInt, usize) {
        let bytes = VarInt::raw_at(bytes, index);
        let varint = VarInt::decode(&bytes);
        (varint, bytes.len())
    }

    /// Reads the bytes of a variable integer.
    /// bytes: A slice of bytes representing the variable integer.
    /// index: The index to start reading the bytes from.
    pub fn raw_at(bytes: &[u8], index: usize) -> Vec<u8> {
        let mut result = vec![];
        for i in index..bytes.len() {
            let byte = bytes[i];
            if byte >> 7 == 1 {
                result.push(byte);
            } else {
                result.push(byte);
                break;
            }
        }

        result
    }

    /// Creates a 32-bit integer representation of the varint.
    pub fn as_i32(&self) -> i32 {
        let mut value = 0;
        self.0.iter().for_each(|byte| {
            value = (value << 7) | *byte as i32;
        });
        value
    }

    /// Creates a 64-bit integer representation of the varint.
    pub fn as_i64(&self) -> i64 {
        let mut value = 0;
        self.0.iter().for_each(|byte| {
            value = (value << 7) | *byte as i64;
        });
        value
    }

    /// Creates a 32-bit unsigned integer representation of the varint.
    /// Returns None if the value is negative.
    pub fn as_u32(&self) -> Option<u32> {
        let value = self.as_i32();
        if value < 0 {
            None
        } else {
            Some(value as u32)
        }
    }

    /// Creates a 64-bit unsigned integer representation of the varint.
    /// Returns None if the value is negative.
    pub fn as_u64(&self) -> Option<u64> {
        let value = self.as_i64();
        if value < 0 {
            None
        } else {
            Some(value as u64)
        }
    }
}

impl Serialize for VarInt {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut i64: Option<i64> = None;
        let mut u32: Option<u32> = None;
        let mut u64: Option<u64> = None;

        // Always serialize i32
        let i32 = self.as_i32();

        // Serialize i64 if there are enough bytes (at least 8 bytes)
        if self.0.len() >= 8 {
            i64 = Some(self.as_i64());

            // Check if the i64 is the same as the i32
            if i64.unwrap() == i32 as i64 {
                i64 = None;
            }
        }

        // Serialize u32 if the value is non-negative
        if let Some(u32_val) = self.as_u32() {
            // If the u32 is the same as the i32, don't serialize it
            if i32 < 0 || i32 as u32 != u32_val {
                u32 = Some(u32_val);

                // Serialize u64 if there are enough bytes (at least 8 bytes) and the value is non-negative
                if self.0.len() >= 8 {
                    if let Some(u64_val) = self.as_u64() {
                        if u64_val != u32_val as u64 {
                            u64 = Some(u64_val);
                        }
                    }
                }
            }
        }

        if i64.is_none() && u32.is_none() && u64.is_none() {
            i32.serialize(serializer)
        } else {
            let mut seq = serializer.serialize_seq(Some(4))?;
            seq.serialize_element(&i32)?;
            if let Some(i64) = i64 {
                seq.serialize_element(&i64)?;
            }
            if let Some(u32) = u32 {
                seq.serialize_element(&u32)?;
            }
            if let Some(u64) = u64 {
                seq.serialize_element(&u64)?;
            }
            seq.end()
        }
    }
}

impl<'de> Deserialize<'de> for VarInt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(VarIntVisitor)
    }
}

struct VarIntVisitor;

impl<'de> Visitor<'de> for VarIntVisitor {
    type Value = VarInt;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a sequence of integers representing a VarInt")
    }

    fn visit_seq<V>(self, mut seq: V) -> Result<VarInt, V::Error>
    where
        V: SeqAccess<'de>,
    {
        let mut bytes = vec![];

        if let Some(i32_val) = seq.next_element::<i32>()? {
            bytes.append(&mut VarInt::encode(i32_val));
        }

        if let Some(i64_val) = seq.next_element::<i64>()? {
            bytes.append(&mut VarInt::encode(i64_val as i32));
        }

        if let Some(u32_val) = seq.next_element::<u32>()? {
            bytes.append(&mut VarInt::encode(u32_val as i32));
        }

        if let Some(u64_val) = seq.next_element::<u64>()? {
            bytes.append(&mut VarInt::encode(u64_val as i32));
        }

        Ok(VarInt(bytes))
    }
}

impl From<VarInt> for i32 {
    fn from(value: VarInt) -> Self {
        value.as_i32()
    }
}

impl From<i32> for VarInt {
    fn from(value: i32) -> Self {
        VarInt(VarInt::encode(value))
    }
}

impl PartialEq<i32> for VarInt {
    fn eq(&self, other: &i32) -> bool {
        self.as_i32() == *other
    }
}

impl PartialEq<VarInt> for i32 {
    fn eq(&self, other: &VarInt) -> bool {
        *self == other.as_i32()
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
