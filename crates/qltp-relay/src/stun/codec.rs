//! STUN Codec for tokio
//!
//! Provides encoding/decoding for STUN messages over UDP/TCP

use bytes::BytesMut;
use tokio_util::codec::{Decoder, Encoder};

use super::message::StunMessage;

/// STUN codec for use with tokio
pub struct StunCodec;

impl Decoder for StunCodec {
    type Item = StunMessage;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 20 {
            // Not enough data for STUN header
            return Ok(None);
        }

        // Parse message length from header (bytes 2-3)
        let length = u16::from_be_bytes([src[2], src[3]]) as usize;
        let total_length = 20 + length;

        if src.len() < total_length {
            // Not enough data for complete message
            return Ok(None);
        }

        // Extract complete message
        let data = src.split_to(total_length).freeze();
        
        // Decode STUN message
        StunMessage::decode(data)
            .map(Some)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }
}

impl Encoder<StunMessage> for StunCodec {
    type Error = std::io::Error;

    fn encode(&mut self, item: StunMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let encoded = item.encode();
        dst.extend_from_slice(&encoded);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codec_encode_decode() {
        let mut codec = StunCodec;
        let msg = StunMessage::binding_request();
        
        let mut buf = BytesMut::new();
        codec.encode(msg.clone(), &mut buf).unwrap();
        
        let decoded = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(decoded.message_type.class, msg.message_type.class);
        assert_eq!(decoded.message_type.method, msg.message_type.method);
    }
}

// Made with Bob
