use tokio::net::tcp::OwnedReadHalf;
use tokio::io::AsyncReadExt;
use sha2::Digest;
use rkyv::rancor::Error;

use crate::net::message::Message;

pub struct MessageReader {
    reader: OwnedReadHalf,
    buffer: Vec<u8>,
}

impl MessageReader {
    pub fn new(reader: OwnedReadHalf) -> Self {
        Self {
            reader,
            buffer: Vec::with_capacity(4096),
        }
    }

    pub async fn next_message(&mut self) -> tokio::io::Result<Message> {
        loop {
            if let Some(msg) = self.try_parse()? {
                return Ok(msg);
            }

            let mut temp = [0u8; 1024];
            let n = self.reader.read(&mut temp).await?;

            if n == 0 {
                return Err(tokio::io::Error::new(
                    tokio::io::ErrorKind::UnexpectedEof,
                    "connection closed",
                ));
            }

            self.buffer.extend_from_slice(&temp[..n]);
        }
    }

    fn try_parse(&mut self) -> tokio::io::Result<Option<Message>> {
        if self.buffer.len() < 8 {
            return Ok(None);
        }

        let length = u64::from_be_bytes(self.buffer[0..8].try_into().unwrap()) as usize;

        if length < 32 || length > 10_000_000 {
            return Err(tokio::io::Error::new(
                tokio::io::ErrorKind::InvalidData,
                format!("invalid length: {}", length),
            ));
        }

        if self.buffer.len() < 8 + length {
            return Ok(None);
        }

        let sha = &self.buffer[8..40];
        let payload = &self.buffer[40..8 + length];

        let computed = sha2::Sha256::digest(payload);
        if computed[..] != sha[..] {
            return Err(tokio::io::Error::new(
                tokio::io::ErrorKind::InvalidData,
                "checksum mismatch",
            ));
        }

        let msg = rkyv::from_bytes::<Message, Error>(payload)
            .map_err(|e| tokio::io::Error::new(tokio::io::ErrorKind::InvalidData, e.to_string()))?;

        self.buffer.drain(0..8 + length);

        Ok(Some(msg))
    }
}