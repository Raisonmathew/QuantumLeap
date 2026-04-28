//! STUN Message Types and Structure
//!
//! Implements RFC 5389 STUN message format

use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::fmt;

use super::{MAGIC_COOKIE, TRANSACTION_ID_SIZE};
use crate::stun::attributes::StunAttribute;

/// STUN message class (2 bits)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StunClass {
    /// Request (0b00)
    Request = 0x00,
    /// Indication (0b01)
    Indication = 0x01,
    /// Success Response (0b10)
    SuccessResponse = 0x02,
    /// Error Response (0b11)
    ErrorResponse = 0x03,
}

impl StunClass {
    /// Parse class from message type bits
    pub fn from_bits(bits: u16) -> Option<Self> {
        let class_bits = ((bits & 0x0100) >> 7) | ((bits & 0x0010) >> 4);
        match class_bits {
            0x00 => Some(StunClass::Request),
            0x01 => Some(StunClass::Indication),
            0x02 => Some(StunClass::SuccessResponse),
            0x03 => Some(StunClass::ErrorResponse),
            _ => None,
        }
    }

    /// Convert class to message type bits
    pub fn to_bits(self) -> u16 {
        match self {
            StunClass::Request => 0x0000,
            StunClass::Indication => 0x0010,
            StunClass::SuccessResponse => 0x0100,
            StunClass::ErrorResponse => 0x0110,
        }
    }
}

/// STUN method (12 bits)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StunMethod {
    /// Binding method (0x001)
    Binding = 0x001,
    /// Allocate method (0x003) - TURN
    Allocate = 0x003,
    /// Refresh method (0x004) - TURN
    Refresh = 0x004,
    /// Send method (0x006) - TURN
    Send = 0x006,
    /// Data method (0x007) - TURN
    Data = 0x007,
    /// CreatePermission method (0x008) - TURN
    CreatePermission = 0x008,
    /// ChannelBind method (0x009) - TURN
    ChannelBind = 0x009,
}

impl StunMethod {
    /// Parse method from message type bits
    pub fn from_bits(bits: u16) -> Option<Self> {
        let method_bits = (bits & 0x000F) | ((bits & 0x00E0) >> 1) | ((bits & 0x3E00) >> 2);
        match method_bits {
            0x001 => Some(StunMethod::Binding),
            0x003 => Some(StunMethod::Allocate),
            0x004 => Some(StunMethod::Refresh),
            0x006 => Some(StunMethod::Send),
            0x007 => Some(StunMethod::Data),
            0x008 => Some(StunMethod::CreatePermission),
            0x009 => Some(StunMethod::ChannelBind),
            _ => None,
        }
    }

    /// Convert method to message type bits
    pub fn to_bits(self) -> u16 {
        let method = self as u16;
        (method & 0x000F) | ((method & 0x0070) << 1) | ((method & 0x0F80) << 2)
    }
}

/// STUN message type (combines class and method)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StunMessageType {
    pub class: StunClass,
    pub method: StunMethod,
}

impl StunMessageType {
    /// Create new message type
    pub fn new(class: StunClass, method: StunMethod) -> Self {
        Self { class, method }
    }

    /// Binding Request
    pub fn binding_request() -> Self {
        Self::new(StunClass::Request, StunMethod::Binding)
    }

    /// Binding Response
    pub fn binding_response() -> Self {
        Self::new(StunClass::SuccessResponse, StunMethod::Binding)
    }

    /// Binding Error Response
    pub fn binding_error() -> Self {
        Self::new(StunClass::ErrorResponse, StunMethod::Binding)
    }

    /// Parse from 14-bit value
    pub fn from_u16(value: u16) -> Option<Self> {
        let class = StunClass::from_bits(value)?;
        let method = StunMethod::from_bits(value)?;
        Some(Self { class, method })
    }

    /// Convert to 14-bit value
    pub fn to_u16(self) -> u16 {
        self.class.to_bits() | self.method.to_bits()
    }
}

impl fmt::Display for StunMessageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} {:?}", self.class, self.method)
    }
}

/// STUN Message
#[derive(Debug, Clone)]
pub struct StunMessage {
    /// Message type (class + method)
    pub message_type: StunMessageType,
    /// Transaction ID (96 bits / 12 bytes)
    pub transaction_id: [u8; TRANSACTION_ID_SIZE],
    /// Message attributes
    pub attributes: Vec<StunAttribute>,
}

impl StunMessage {
    /// Create new STUN message
    pub fn new(message_type: StunMessageType, transaction_id: [u8; TRANSACTION_ID_SIZE]) -> Self {
        Self {
            message_type,
            transaction_id,
            attributes: Vec::new(),
        }
    }

    /// Create Binding Request with random transaction ID
    pub fn binding_request() -> Self {
        let mut transaction_id = [0u8; TRANSACTION_ID_SIZE];
        rand::Rng::fill(&mut rand::thread_rng(), &mut transaction_id);
        Self::new(StunMessageType::binding_request(), transaction_id)
    }

    /// Create Binding Response
    pub fn binding_response(transaction_id: [u8; TRANSACTION_ID_SIZE]) -> Self {
        Self::new(StunMessageType::binding_response(), transaction_id)
    }

    /// Add attribute
    pub fn add_attribute(&mut self, attribute: StunAttribute) {
        self.attributes.push(attribute);
    }

    /// Get attribute by type
    pub fn get_attribute(&self, attr_type: u16) -> Option<&StunAttribute> {
        self.attributes.iter().find(|attr| attr.attr_type() == attr_type)
    }

    /// Calculate message length (excluding 20-byte header)
    pub fn calculate_length(&self) -> u16 {
        self.attributes.iter().map(|attr| attr.encoded_length()).sum()
    }

    /// Encode message to bytes
    pub fn encode(&self) -> Bytes {
        let length = self.calculate_length();
        let mut buf = BytesMut::with_capacity(20 + length as usize);

        // Message Type (14 bits) + Reserved (2 bits, must be 0)
        buf.put_u16(self.message_type.to_u16());

        // Message Length (16 bits)
        buf.put_u16(length);

        // Magic Cookie (32 bits)
        buf.put_u32(MAGIC_COOKIE);

        // Transaction ID (96 bits)
        buf.put_slice(&self.transaction_id);

        // Attributes
        for attr in &self.attributes {
            attr.encode(&mut buf);
        }

        buf.freeze()
    }

    /// Decode message from bytes
    pub fn decode(mut data: Bytes) -> Result<Self, String> {
        if data.len() < 20 {
            return Err("Message too short".to_string());
        }

        // Parse message type
        let msg_type_bits = data.get_u16();
        let message_type = StunMessageType::from_u16(msg_type_bits)
            .ok_or("Invalid message type")?;

        // Parse length
        let length = data.get_u16();

        // Verify magic cookie
        let magic = data.get_u32();
        if magic != MAGIC_COOKIE {
            return Err(format!("Invalid magic cookie: 0x{:08X}", magic));
        }

        // Parse transaction ID
        let mut transaction_id = [0u8; TRANSACTION_ID_SIZE];
        data.copy_to_slice(&mut transaction_id);

        // Parse attributes
        //
        // SECURITY (CWE-835, infinite loop): each iteration MUST consume at
        // least 4 bytes from the message length budget (the attribute TLV
        // header). If `encoded_length()` ever returned 0 (or any value <4)
        // the previous `saturating_sub` would leave `remaining` unchanged
        // and the loop would spin forever on a crafted packet. We now
        // enforce strict forward progress: an iteration that consumes less
        // than the minimum header size is treated as a malformed message
        // and rejected.
        const ATTR_HEADER_SIZE: usize = 4;
        let mut attributes = Vec::new();
        let mut remaining = length as usize;

        while remaining > 0 {
            if data.remaining() < ATTR_HEADER_SIZE {
                return Err("Incomplete attribute header".to_string());
            }

            let attr = StunAttribute::decode(&mut data)?;
            let consumed = attr.encoded_length() as usize;
            if consumed < ATTR_HEADER_SIZE {
                return Err(format!(
                    "Malformed STUN attribute: encoded_length {} < header size {}",
                    consumed, ATTR_HEADER_SIZE
                ));
            }
            if consumed > remaining {
                return Err(format!(
                    "STUN attribute consumed {} bytes past message length budget",
                    consumed - remaining
                ));
            }
            remaining -= consumed;
            attributes.push(attr);
        }

        Ok(Self {
            message_type,
            transaction_id,
            attributes,
        })
    }

    /// Check if this is a request
    pub fn is_request(&self) -> bool {
        self.message_type.class == StunClass::Request
    }

    /// Check if this is a response
    pub fn is_response(&self) -> bool {
        matches!(
            self.message_type.class,
            StunClass::SuccessResponse | StunClass::ErrorResponse
        )
    }

    /// Check if this is an indication
    pub fn is_indication(&self) -> bool {
        self.message_type.class == StunClass::Indication
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stun_class_bits() {
        assert_eq!(StunClass::Request.to_bits(), 0x0000);
        assert_eq!(StunClass::Indication.to_bits(), 0x0010);
        assert_eq!(StunClass::SuccessResponse.to_bits(), 0x0100);
        assert_eq!(StunClass::ErrorResponse.to_bits(), 0x0110);
    }

    #[test]
    fn test_stun_method_bits() {
        assert_eq!(StunMethod::Binding.to_bits(), 0x0001);
    }

    #[test]
    fn test_message_type_encoding() {
        let msg_type = StunMessageType::binding_request();
        let bits = msg_type.to_u16();
        assert_eq!(bits, 0x0001); // Binding Request

        let decoded = StunMessageType::from_u16(bits).unwrap();
        assert_eq!(decoded.class, StunClass::Request);
        assert_eq!(decoded.method, StunMethod::Binding);
    }

    #[test]
    fn test_binding_request_creation() {
        let msg = StunMessage::binding_request();
        assert!(msg.is_request());
        assert_eq!(msg.message_type.method, StunMethod::Binding);
        assert_eq!(msg.attributes.len(), 0);
    }

    #[test]
    fn test_message_encode_decode() {
        let mut msg = StunMessage::binding_request();
        let encoded = msg.encode();
        
        assert_eq!(encoded.len(), 20); // Header only, no attributes
        
        let decoded = StunMessage::decode(encoded).unwrap();
        assert_eq!(decoded.message_type.class, msg.message_type.class);
        assert_eq!(decoded.message_type.method, msg.message_type.method);
        assert_eq!(decoded.transaction_id, msg.transaction_id);
    }
}

// Made with Bob
