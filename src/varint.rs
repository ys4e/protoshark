use std::fmt;
use paste::paste;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde::de::{SeqAccess, Visitor};
use serde::ser::SerializeSeq;

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

    /// Encodes a 32-bit integer into a variable integer.
    /// value: The 32-bit integer to encode.
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

    /// Encodes a 64-bit integer into a variable integer.
    /// value: The 64-bit integer to encode.
    pub fn encode_long(value: i64) -> Vec<u8> {
        let mut bytes = vec![];
        for i in 0..10 {
            let mut byte = (value >> (i * 7)) as u8;
            if i == 9 {
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

    /// Returns the length of the buffer for the varint.
    pub fn length(&self) -> usize {
        self.0.len()
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

/// This trait allows primitive numbers to be converted into
/// variable-length integers encoded as a byte array.
pub trait IntoVarInt {
    /// Encodes the value into a byte array.
    /// 
    /// The byte array represents a variable-length integer.
    fn into_varint(self) -> Vec<u8>;
}

impl IntoVarInt for u32 {
    fn into_varint(self) -> Vec<u8> {
        VarInt::encode(self as i32)
    }
}

impl IntoVarInt for u64 {
    fn into_varint(self) -> Vec<u8> {
        VarInt::encode_long(self as i64)
    }
}

/// Generates the implementations for traits to convert
/// and compare variable integers to their Rust primitives.
macro_rules! impl_varint {
    ($($target:ty => $encoder:ident),*) => {
        $(
            impl From<$target> for VarInt {
                fn from(value: $target) -> Self {
                    VarInt(VarInt::$encoder(value))
                }
            }
            
            impl IntoVarInt for $target {
                fn into_varint(self) -> Vec<u8> {
                    VarInt::$encoder(self)
                }
            }

            paste! {
                impl From<VarInt> for $target {
                    fn from(value: VarInt) -> Self {
                        value.[<as_ $target>]()
                    }
                }

                impl PartialEq<$target> for VarInt {
                    fn eq(&self, other: &$target) -> bool {
                        self.[<as_ $target>]() == *other
                    }
                }

                impl PartialEq<VarInt> for $target {
                    fn eq(&self, other: &VarInt) -> bool {
                        *self == other.[<as_ $target>]()
                    }
                }
            }
        )*
    };
}

impl_varint!(
    i32 => encode,
    i64 => encode_long
);