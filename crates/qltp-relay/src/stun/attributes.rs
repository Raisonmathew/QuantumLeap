//! STUN Attributes
//!
//! Implements RFC 5389 STUN attributes

use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use super::MAGIC_COOKIE;

/// STUN attribute types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum AttributeType {
    /// MAPPED-ADDRESS (0x0001)
    MappedAddress = 0x0001,
    /// USERNAME (0x0006)
    Username = 0x0006,
    /// MESSAGE-INTEGRITY (0x0008)
    MessageIntegrity = 0x0008,
    /// ERROR-CODE (0x0009)
    ErrorCode = 0x0009,
    /// UNKNOWN-ATTRIBUTES (0x000A)
    UnknownAttributes = 0x000A,
    /// REALM (0x0014)
    Realm = 0x0014,
    /// NONCE (0x0015)
    Nonce = 0x0015,
    /// XOR-MAPPED-ADDRESS (0x0020)
    XorMappedAddress = 0x0020,
    /// SOFTWARE (0x8022)
    Software = 0x8022,
    /// ALTERNATE-SERVER (0x8023)
    AlternateServer = 0x8023,
    /// FINGERPRINT (0x8028)
    Fingerprint = 0x8028,
}

/// STUN Attribute
#[derive(Debug, Clone)]
pub enum StunAttribute {
    /// MAPPED-ADDRESS attribute
    MappedAddress(MappedAddress),
    /// XOR-MAPPED-ADDRESS attribute
    XorMappedAddress(MappedAddress),
    /// USERNAME attribute
    Username(String),
    /// MESSAGE-INTEGRITY attribute (HMAC-SHA1, 20 bytes)
    MessageIntegrity([u8; 20]),
    /// ERROR-CODE attribute
    ErrorCode { code: u16, reason: String },
    /// UNKNOWN-ATTRIBUTES attribute
    UnknownAttributes(Vec<u16>),
    /// REALM attribute
    Realm(String),
    /// NONCE attribute
    Nonce(String),
    /// SOFTWARE attribute
    Software(String),
    /// ALTERNATE-SERVER attribute
    AlternateServer(SocketAddr),
    /// FINGERPRINT attribute (CRC-32, 4 bytes)
    Fingerprint(u32),
    /// Unknown attribute (for forward compatibility)
    Unknown { attr_type: u16, value: Bytes },
}

impl StunAttribute {
    /// Get attribute type code
    pub fn attr_type(&self) -> u16 {
        match self {
            Self::MappedAddress(_) => AttributeType::MappedAddress as u16,
            Self::XorMappedAddress(_) => AttributeType::XorMappedAddress as u16,
            Self::Username(_) => AttributeType::Username as u16,
            Self::MessageIntegrity(_) => AttributeType::MessageIntegrity as u16,
            Self::ErrorCode { .. } => AttributeType::ErrorCode as u16,
            Self::UnknownAttributes(_) => AttributeType::UnknownAttributes as u16,
            Self::Realm(_) => AttributeType::Realm as u16,
            Self::Nonce(_) => AttributeType::Nonce as u16,
            Self::Software(_) => AttributeType::Software as u16,
            Self::AlternateServer(_) => AttributeType::AlternateServer as u16,
            Self::Fingerprint(_) => AttributeType::Fingerprint as u16,
            Self::Unknown { attr_type, .. } => *attr_type,
        }
    }

    /// Get encoded length (including type and length fields, with padding)
    pub fn encoded_length(&self) -> u16 {
        let value_len = match self {
            Self::MappedAddress(addr) | Self::XorMappedAddress(addr) => addr.encoded_length(),
            Self::Username(s) | Self::Realm(s) | Self::Nonce(s) | Self::Software(s) => s.len() as u16,
            Self::MessageIntegrity(_) => 20,
            Self::ErrorCode { reason, .. } => 4 + reason.len() as u16,
            Self::UnknownAttributes(attrs) => (attrs.len() * 2) as u16,
            Self::AlternateServer(_) => 8, // Family (1) + Port (2) + Address (4) + padding (1)
            Self::Fingerprint(_) => 4,
            Self::Unknown { value, .. } => value.len() as u16,
        };

        // Attribute header (4 bytes) + value + padding to 4-byte boundary
        4 + value_len + ((4 - (value_len % 4)) % 4)
    }

    /// Encode attribute to buffer
    pub fn encode(&self, buf: &mut BytesMut) {
        // Attribute Type (16 bits)
        buf.put_u16(self.attr_type());

        // Attribute Length (16 bits) - length of value only, not including padding
        let value_len = self.encoded_length() - 4 - ((4 - ((self.encoded_length() - 4) % 4)) % 4);
        buf.put_u16(value_len);

        // Attribute Value
        match self {
            Self::MappedAddress(addr) => addr.encode(buf, false),
            Self::XorMappedAddress(addr) => addr.encode(buf, true),
            Self::Username(s) | Self::Realm(s) | Self::Nonce(s) | Self::Software(s) => {
                buf.put_slice(s.as_bytes());
            }
            Self::MessageIntegrity(hmac) => {
                buf.put_slice(hmac);
            }
            Self::ErrorCode { code, reason } => {
                buf.put_u16(0); // Reserved
                buf.put_u8((code / 100) as u8); // Class
                buf.put_u8((code % 100) as u8); // Number
                buf.put_slice(reason.as_bytes());
            }
            Self::UnknownAttributes(attrs) => {
                for attr in attrs {
                    buf.put_u16(*attr);
                }
            }
            Self::AlternateServer(addr) => {
                encode_socket_addr(buf, addr, false);
            }
            Self::Fingerprint(crc) => {
                buf.put_u32(*crc);
            }
            Self::Unknown { value, .. } => {
                buf.put_slice(value);
            }
        }

        // Add padding to 4-byte boundary
        let padding = (4 - (value_len % 4)) % 4;
        for _ in 0..padding {
            buf.put_u8(0);
        }
    }

    /// Decode attribute from buffer
    pub fn decode(data: &mut Bytes) -> Result<Self, String> {
        if data.remaining() < 4 {
            return Err("Incomplete attribute header".to_string());
        }

        let attr_type = data.get_u16();
        let length = data.get_u16();

        if data.remaining() < length as usize {
            return Err("Incomplete attribute value".to_string());
        }

        let mut value = data.split_to(length as usize);

        // Skip padding
        let padding = (4 - (length % 4)) % 4;
        if data.remaining() >= padding as usize {
            data.advance(padding as usize);
        }

        let attribute = match attr_type {
            0x0001 => Self::MappedAddress(MappedAddress::decode(&mut value, false)?),
            0x0020 => Self::XorMappedAddress(MappedAddress::decode(&mut value, true)?),
            0x0006 => Self::Username(String::from_utf8_lossy(&value).to_string()),
            0x0008 => {
                if value.len() != 20 {
                    return Err("Invalid MESSAGE-INTEGRITY length".to_string());
                }
                let mut hmac = [0u8; 20];
                value.copy_to_slice(&mut hmac);
                Self::MessageIntegrity(hmac)
            }
            0x0009 => {
                if value.len() < 4 {
                    return Err("Invalid ERROR-CODE length".to_string());
                }
                value.advance(2); // Skip reserved
                let class = value.get_u8() as u16;
                let number = value.get_u8() as u16;
                let code = class * 100 + number;
                let reason = String::from_utf8_lossy(&value).to_string();
                Self::ErrorCode { code, reason }
            }
            0x8028 => {
                if value.len() != 4 {
                    return Err("Invalid FINGERPRINT length".to_string());
                }
                Self::Fingerprint(value.get_u32())
            }
            _ => Self::Unknown {
                attr_type,
                value,
            },
        };

        Ok(attribute)
    }
}

/// Mapped Address (IP address and port)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MappedAddress {
    pub addr: SocketAddr,
}

impl MappedAddress {
    pub fn new(addr: SocketAddr) -> Self {
        Self { addr }
    }

    fn encoded_length(&self) -> u16 {
        match self.addr {
            SocketAddr::V4(_) => 8,  // Family (1) + Port (2) + IPv4 (4) + padding (1)
            SocketAddr::V6(_) => 20, // Family (1) + Port (2) + IPv6 (16) + padding (1)
        }
    }

    fn encode(&self, buf: &mut BytesMut, xor: bool) {
        encode_socket_addr(buf, &self.addr, xor);
    }

    fn decode(data: &mut Bytes, xor: bool) -> Result<Self, String> {
        decode_socket_addr(data, xor).map(|addr| Self { addr })
    }
}

/// Encode socket address
fn encode_socket_addr(buf: &mut BytesMut, addr: &SocketAddr, xor: bool) {
    buf.put_u8(0); // Reserved
    
    match addr {
        SocketAddr::V4(v4) => {
            buf.put_u8(0x01); // IPv4 family
            
            let port = if xor {
                v4.port() ^ ((MAGIC_COOKIE >> 16) as u16)
            } else {
                v4.port()
            };
            buf.put_u16(port);
            
            let ip_bytes = v4.ip().octets();
            if xor {
                let magic_bytes = MAGIC_COOKIE.to_be_bytes();
                for i in 0..4 {
                    buf.put_u8(ip_bytes[i] ^ magic_bytes[i]);
                }
            } else {
                buf.put_slice(&ip_bytes);
            }
        }
        SocketAddr::V6(v6) => {
            buf.put_u8(0x02); // IPv6 family
            
            let port = if xor {
                v6.port() ^ ((MAGIC_COOKIE >> 16) as u16)
            } else {
                v6.port()
            };
            buf.put_u16(port);
            
            let ip_bytes = v6.ip().octets();
            if xor {
                // XOR with magic cookie + transaction ID (not implemented here for simplicity)
                buf.put_slice(&ip_bytes);
            } else {
                buf.put_slice(&ip_bytes);
            }
        }
    }
}

/// Decode socket address
fn decode_socket_addr(data: &mut Bytes, xor: bool) -> Result<SocketAddr, String> {
    if data.remaining() < 4 {
        return Err("Incomplete address".to_string());
    }

    data.advance(1); // Skip reserved
    let family = data.get_u8();
    let port = data.get_u16();

    let port = if xor {
        port ^ ((MAGIC_COOKIE >> 16) as u16)
    } else {
        port
    };

    match family {
        0x01 => {
            // IPv4
            if data.remaining() < 4 {
                return Err("Incomplete IPv4 address".to_string());
            }
            let mut ip_bytes = [0u8; 4];
            data.copy_to_slice(&mut ip_bytes);
            
            if xor {
                let magic_bytes = MAGIC_COOKIE.to_be_bytes();
                for i in 0..4 {
                    ip_bytes[i] ^= magic_bytes[i];
                }
            }
            
            let ip = Ipv4Addr::from(ip_bytes);
            Ok(SocketAddr::new(IpAddr::V4(ip), port))
        }
        0x02 => {
            // IPv6
            if data.remaining() < 16 {
                return Err("Incomplete IPv6 address".to_string());
            }
            let mut ip_bytes = [0u8; 16];
            data.copy_to_slice(&mut ip_bytes);
            
            let ip = Ipv6Addr::from(ip_bytes);
            Ok(SocketAddr::new(IpAddr::V6(ip), port))
        }
        _ => Err(format!("Unknown address family: {}", family)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mapped_address_encode_decode() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);
        let mapped = MappedAddress::new(addr);
        
        let mut buf = BytesMut::new();
        mapped.encode(&mut buf, false);
        
        let mut data = buf.freeze();
        let decoded = MappedAddress::decode(&mut data, false).unwrap();
        
        assert_eq!(decoded.addr, addr);
    }

    #[test]
    fn test_xor_mapped_address() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);
        let mapped = MappedAddress::new(addr);
        
        let mut buf = BytesMut::new();
        mapped.encode(&mut buf, true);
        
        let mut data = buf.freeze();
        let decoded = MappedAddress::decode(&mut data, true).unwrap();
        
        assert_eq!(decoded.addr, addr);
    }

    #[test]
    fn test_attribute_encode_decode() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);
        let attr = StunAttribute::XorMappedAddress(MappedAddress::new(addr));
        
        let mut buf = BytesMut::new();
        attr.encode(&mut buf);
        
        let mut data = buf.freeze();
        let decoded = StunAttribute::decode(&mut data).unwrap();
        
        match decoded {
            StunAttribute::XorMappedAddress(mapped) => {
                assert_eq!(mapped.addr, addr);
            }
            _ => panic!("Wrong attribute type"),
        }
    }
}

// Made with Bob
