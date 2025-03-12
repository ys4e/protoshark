use paste::paste;
use crate::{Header, IntoVarInt, VarInt, WireType};

/// A macro to write a header to the byte array.
macro_rules! h {
    ($field:ident, $t:expr) => {
        Header::new($field, $t).to_bytes()
    };
}

/// A macro to generate implementations for the `write_<prim>` functions.
macro_rules! impl_encode {
    ($($t:tt),*) => {
        $(
            paste! {
                fn [<write_ $t>](&mut self, field: u32, value: $t) {
                    self.extend(h!(field, WireType::VarInt));
                    self.extend(value.into_varint());
                }
            }
        )*
    };
}

/// A trait to be implemented on heap-allocated byte arrays.
///
/// Contains helpful utilities for encoding/decoding protobuf types.
pub trait ProtobufBytes {
    /// Writes a series of bytes to the byte array.
    fn write_bytes(&mut self, field: u32, value: &[u8]);
    
    /// Writes a string to the byte array.
    fn write_str(&mut self, field: u32, value: &str);

    /// Writes a `u32` variable-length integer to the byte array.
    fn write_u32(&mut self, field: u32, value: u32);

    /// Writes a `u64` variable-length integer to the byte array.
    fn write_u64(&mut self, field: u32, value: u64);
    
    /// Writes a `i32` variable-length integer to the byte array.
    fn write_i32(&mut self, field: u32, value: i32);
    
    /// Writes a `i64` variable-length integer to the byte array.
    fn write_i64(&mut self, field: u32, value: i64);
    
    /// Writes a `f32` fixed-length floating point decimal to the byte array.
    fn write_f32(&mut self, field: u32, value: f32);

    /// Writes a `f64` fixed-length floating point decimal to the byte array.
    fn write_f64(&mut self, field: u32, value: f64);
}

impl ProtobufBytes for Vec<u8> {
    fn write_bytes(&mut self, field: u32, value: &[u8]) {
        self.extend(h!(field, WireType::LengthDelimited));
        self.extend(VarInt::encode(value.len() as i32));
        self.extend(value);
    }

    fn write_str(&mut self, field: u32, value: &str) {
        self.write_bytes(field, value.as_bytes());
    }

    impl_encode!(i32, i64, u32, u64);

    fn write_f32(&mut self, field: u32, value: f32) {
        self.extend(h!(field, WireType::Fixed32));
        self.extend(value.to_le_bytes());
    }

    fn write_f64(&mut self, field: u32, value: f64) {
        self.extend(h!(field, WireType::Fixed64));
        self.extend(value.to_le_bytes());
    }
}