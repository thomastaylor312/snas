use std::fmt::{Debug, Display};

use bincode::{BorrowDecode, Decode, Encode};
use serde::{Deserialize, Serialize};

/// A string wrapper type that will not leak credentials in logs or printing while still able to be
/// used as a string. Will zero out the memory when dropped.
#[derive(Clone, PartialEq, Eq)]
pub struct SecureString(String);

impl Drop for SecureString {
    fn drop(&mut self) {
        // SAFETY: We're dropping so writing zeros to this vec is fine.
        unsafe {
            for b in self.0.as_mut_vec() {
                *b = 0;
            }
        }
    }
}

impl Debug for SecureString {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "**********")
    }
}

impl Display for SecureString {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "**********")
    }
}

impl Encode for SecureString {
    fn encode<E: bincode::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), bincode::error::EncodeError> {
        Encode::encode(&self.0, encoder)
    }
}

impl Decode for SecureString {
    fn decode<D: bincode::de::Decoder>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        let s: String = Decode::decode(decoder)?;
        Ok(Self(s))
    }
}

impl<'de> BorrowDecode<'de> for SecureString {
    fn borrow_decode<D: bincode::de::BorrowDecoder<'de>>(
        decoder: &mut D,
    ) -> core::result::Result<Self, bincode::error::DecodeError> {
        let s: String = BorrowDecode::borrow_decode(decoder)?;
        Ok(Self(s))
    }
}

impl Serialize for SecureString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_ref())
    }
}

impl<'de> Deserialize<'de> for SecureString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Self(s))
    }
}

impl From<String> for SecureString {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for SecureString {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl AsRef<str> for SecureString {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl AsRef<String> for SecureString {
    fn as_ref(&self) -> &String {
        &self.0
    }
}

/// A vec of bytes wrapper that will not leak credentials in logs or printing while still able to be
/// used as a Vec<u8>. Will zero out the memory when dropped.
#[derive(Clone, PartialEq, Eq)]
pub struct SecureBytes(Vec<u8>);

impl Drop for SecureBytes {
    fn drop(&mut self) {
        for b in self.0.iter_mut() {
            *b = 0;
        }
    }
}

impl Debug for SecureBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "**********")
    }
}

impl Display for SecureBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "**********")
    }
}

impl Encode for SecureBytes {
    fn encode<E: bincode::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), bincode::error::EncodeError> {
        Encode::encode(&self.0, encoder)
    }
}

impl Decode for SecureBytes {
    fn decode<D: bincode::de::Decoder>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        let b: Vec<u8> = Decode::decode(decoder)?;
        Ok(Self(b))
    }
}

impl<'de> BorrowDecode<'de> for SecureBytes {
    fn borrow_decode<D: bincode::de::BorrowDecoder<'de>>(
        decoder: &mut D,
    ) -> core::result::Result<Self, bincode::error::DecodeError> {
        let s: &[u8] = BorrowDecode::borrow_decode(decoder)?;
        Ok(Self(s.to_vec()))
    }
}

impl Serialize for SecureBytes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serde_bytes::serialize(&self.0, serializer)
    }
}

impl<'de> Deserialize<'de> for SecureBytes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let b: Vec<u8> = serde_bytes::deserialize(deserializer)?;
        Ok(Self(b))
    }
}

impl From<Vec<u8>> for SecureBytes {
    fn from(s: Vec<u8>) -> Self {
        Self(s)
    }
}

impl From<&[u8]> for SecureBytes {
    fn from(s: &[u8]) -> Self {
        Self(s.to_vec())
    }
}

impl AsRef<[u8]> for SecureBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl AsRef<Vec<u8>> for SecureBytes {
    fn as_ref(&self) -> &Vec<u8> {
        &self.0
    }
}
