//! Message codec for serialization and framing

use super::messages::{Message, MessageHeader, MessageType, MAX_PAYLOAD_SIZE};
use bytes::{Buf, BufMut, BytesMut};
use crc32fast::Hasher;
use std::io;
use tokio_util::codec::{Decoder, Encoder};
use uuid::Uuid;

/// QLTP message codec for framing and serialization
pub struct QltpCodec {
    session_id: Uuid,
    sequence_number: u32,
}

impl QltpCodec {
    pub fn new(session_id: Uuid) -> Self {
        Self {
            session_id,
            sequence_number: 0,
        }
    }

    fn calculate_checksum(data: &[u8]) -> u32 {
        let mut hasher = Hasher::new();
        hasher.update(data);
        hasher.finalize()
    }

    fn encode_header(
        &mut self,
        message_type: MessageType,
        payload_length: u32,
        checksum: u32,
    ) -> Vec<u8> {
        let mut buf = Vec::with_capacity(30);
        
        // Message type (1 byte)
        buf.put_u8(message_type as u8);
        
        // Flags (1 byte)
        buf.put_u8(0);
        
        // Sequence number (4 bytes)
        buf.put_u32(self.sequence_number);
        self.sequence_number = self.sequence_number.wrapping_add(1);
        
        // Session ID (16 bytes)
        buf.extend_from_slice(self.session_id.as_bytes());
        
        // Payload length (4 bytes)
        buf.put_u32(payload_length);
        
        // Checksum (4 bytes)
        buf.put_u32(checksum);
        
        buf
    }

    fn decode_header(src: &mut BytesMut) -> io::Result<MessageHeader> {
        if src.len() < 30 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Incomplete header",
            ));
        }

        let message_type = MessageType::from_u8(src.get_u8()).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "Invalid message type")
        })?;

        let flags = src.get_u8();
        let sequence_number = src.get_u32();

        let mut session_id_bytes = [0u8; 16];
        src.copy_to_slice(&mut session_id_bytes);
        let session_id = Uuid::from_bytes(session_id_bytes);

        let payload_length = src.get_u32();
        let checksum = src.get_u32();

        if payload_length > MAX_PAYLOAD_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Payload too large",
            ));
        }

        Ok(MessageHeader {
            message_type,
            flags,
            sequence_number,
            session_id,
            payload_length,
            checksum,
        })
    }
}

impl Decoder for QltpCodec {
    type Item = Message;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // Need at least header
        if src.len() < 30 {
            return Ok(None);
        }

        // Parse header without consuming
        let mut peek = src.clone();
        let header = Self::decode_header(&mut peek)?;

        // Check if we have the full message
        let total_length = 30 + header.payload_length as usize;
        if src.len() < total_length {
            // Reserve space for the full message
            src.reserve(total_length - src.len());
            return Ok(None);
        }

        // Now consume the header
        let consumed_header = Self::decode_header(src)?;
        
        // Check if we have enough bytes for payload
        if src.len() < consumed_header.payload_length as usize {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!("Not enough bytes for payload: have {}, need {}", src.len(), consumed_header.payload_length),
            ));
        }

        // Get payload
        let payload = src.split_to(consumed_header.payload_length as usize);

        // Verify checksum
        let calculated_checksum = Self::calculate_checksum(&payload);
        if calculated_checksum != consumed_header.checksum {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Checksum mismatch",
            ));
        }

        // Deserialize message based on type
        let message = match consumed_header.message_type {
            MessageType::Hello => {
                let msg: super::messages::HelloMessage = bincode::deserialize(&payload)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                Message::Hello(msg)
            }
            MessageType::Welcome => {
                let msg: super::messages::WelcomeMessage = bincode::deserialize(&payload)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                Message::Welcome(msg)
            }
            MessageType::TransferStart => {
                let msg: super::messages::TransferStartMessage = bincode::deserialize(&payload)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                Message::TransferStart(msg)
            }
            MessageType::TransferAck => {
                let msg: super::messages::TransferAckMessage = bincode::deserialize(&payload)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                Message::TransferAck(msg)
            }
            MessageType::ChunkData => {
                let msg: super::messages::ChunkDataMessage = bincode::deserialize(&payload)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                Message::ChunkData(msg)
            }
            MessageType::ChunkAck => {
                let msg: super::messages::ChunkAckMessage = bincode::deserialize(&payload)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                Message::ChunkAck(msg)
            }
            MessageType::TransferEnd => {
                let msg: super::messages::TransferEndMessage = bincode::deserialize(&payload)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                Message::TransferEnd(msg)
            }
            MessageType::TransferComplete => {
                let msg: super::messages::TransferCompleteMessage = bincode::deserialize(&payload)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                Message::TransferComplete(msg)
            }
            MessageType::Error => {
                let msg: super::messages::ErrorMessage = bincode::deserialize(&payload)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                Message::Error(msg)
            }
            MessageType::Ping => Message::Ping,
            MessageType::Pong => Message::Pong,
            MessageType::ResumeRequest => {
                let msg: super::messages::ResumeRequestMessage = bincode::deserialize(&payload)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                Message::ResumeRequest(msg)
            }
            MessageType::ResumeAck => {
                let msg: super::messages::ResumeAckMessage = bincode::deserialize(&payload)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                Message::ResumeAck(msg)
            }
            MessageType::Goodbye => Message::Goodbye,
            MessageType::Metadata => {
                // Not implemented yet
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "Metadata message not implemented",
                ));
            }
        };

        Ok(Some(message))
    }
}

impl Encoder<Message> for QltpCodec {
    type Error = io::Error;

    fn encode(&mut self, item: Message, dst: &mut BytesMut) -> Result<(), Self::Error> {
        // Serialize payload based on message type
        let (message_type, payload) = match &item {
            Message::Hello(msg) => (MessageType::Hello,
                bincode::serialize(msg).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?),
            Message::Welcome(msg) => (MessageType::Welcome,
                bincode::serialize(msg).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?),
            Message::TransferStart(msg) => (MessageType::TransferStart,
                bincode::serialize(msg).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?),
            Message::TransferAck(msg) => (MessageType::TransferAck,
                bincode::serialize(msg).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?),
            Message::ChunkData(msg) => (MessageType::ChunkData,
                bincode::serialize(msg).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?),
            Message::ChunkAck(msg) => (MessageType::ChunkAck,
                bincode::serialize(msg).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?),
            Message::TransferEnd(msg) => (MessageType::TransferEnd,
                bincode::serialize(msg).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?),
            Message::TransferComplete(msg) => (MessageType::TransferComplete,
                bincode::serialize(msg).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?),
            Message::Error(msg) => (MessageType::Error,
                bincode::serialize(msg).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?),
            Message::Ping => (MessageType::Ping, Vec::new()),
            Message::Pong => (MessageType::Pong, Vec::new()),
            Message::ResumeRequest(msg) => (MessageType::ResumeRequest,
                bincode::serialize(msg).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?),
            Message::ResumeAck(msg) => (MessageType::ResumeAck,
                bincode::serialize(msg).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?),
            Message::Goodbye => (MessageType::Goodbye, Vec::new()),
        };

        if payload.len() > MAX_PAYLOAD_SIZE as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Payload too large",
            ));
        }

        // Calculate checksum
        let checksum = Self::calculate_checksum(&payload);

        // Encode header
        let header = self.encode_header(
            message_type,
            payload.len() as u32,
            checksum,
        );

        // Reserve space
        dst.reserve(header.len() + payload.len());

        // Write header and payload
        dst.extend_from_slice(&header);
        dst.extend_from_slice(&payload);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::messages::{Capabilities, HelloMessage};

    #[test]
    fn test_codec_roundtrip() {
        let session_id = Uuid::new_v4();
        let mut encoder = QltpCodec::new(session_id);
        let mut decoder = QltpCodec::new(session_id);

        // Create a message
        let client_id = Uuid::new_v4();
        let caps = Capabilities::default_client();
        let hello = HelloMessage::new(client_id, caps);
        let message = Message::Hello(hello.clone());

        // Encode
        let mut buf = BytesMut::new();
        encoder.encode(message, &mut buf).unwrap();

        // Decode
        let decoded = decoder.decode(&mut buf).unwrap();
        assert!(decoded.is_some());

        if let Some(Message::Hello(decoded_hello)) = decoded {
            assert_eq!(decoded_hello.client_id, hello.client_id);
            assert_eq!(decoded_hello.magic, hello.magic);
        } else {
            panic!("Expected Hello message");
        }
    }

    #[test]
    fn test_incomplete_message() {
        let session_id = Uuid::new_v4();
        let mut decoder = QltpCodec::new(session_id);

        // Only header, no payload
        let mut buf = BytesMut::new();
        buf.put_u8(0x01); // Message type
        buf.put_u8(0x00); // Flags
        buf.put_u32(0); // Sequence
        buf.extend_from_slice(session_id.as_bytes());
        buf.put_u32(100); // Payload length
        buf.put_u32(0); // Checksum

        // Should return None (incomplete)
        let result = decoder.decode(&mut buf).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_checksum_validation() {
        let session_id = Uuid::new_v4();
        let mut encoder = QltpCodec::new(session_id);
        let mut decoder = QltpCodec::new(session_id);

        let client_id = Uuid::new_v4();
        let caps = Capabilities::default_client();
        let hello = HelloMessage::new(client_id, caps);
        let message = Message::Hello(hello);

        // Encode
        let mut buf = BytesMut::new();
        encoder.encode(message, &mut buf).unwrap();

        // Corrupt checksum
        let len = buf.len();
        buf[26] ^= 0xFF; // Flip bits in checksum

        // Decode should fail
        let result = decoder.decode(&mut buf);
        assert!(result.is_err());
    }
}

// Made with Bob