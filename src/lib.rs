#![doc = include_str!("../README.md")]
#![deny(unsafe_code, missing_docs, clippy::unwrap_used)]

pub mod cache;
pub mod handler;
pub mod log;
pub mod storage;
pub mod weixin;

#[cfg(test)]
mod tests {}
