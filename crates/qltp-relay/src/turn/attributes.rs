//! TURN-specific Attributes
//!
//! Implements RFC 5766 TURN attributes

use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use crate::stun::MAGIC_COOKIE;

/// TURN attribute types (extends STUN attributes)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum TurnAttributeType {
    /// CHANNEL-NUMBER (0x000C)
    ChannelNumber = 0x000C,
    /// LIFETIME (0x000D)
    Lifetime = 0x000D,
    /// XOR-PEER-ADDRESS (0x0012)
    XorPeerAddress = 0x0012,
    /// DATA (0x0013)
    Data = 0x0013,
    /// XOR-RELAYED-ADDRESS (0x0016)
    XorRelayedAddress = 0x0016,
    /// EVEN-PORT (0x0018)
    EvenPort = 0x0018,
    /// REQUESTED-TRANSPORT (0x0019)
    RequestedTransport = 0x0019,
    /// DONT-FRAGMENT (0x001A)
    DontFragment = 0x001A,
    /// RESERVATION-TOKEN (0x0022)
    ReservationToken = 0x0022,
}

/// TURN Attribute
#[derive(Debug, Clone)]
pub enum TurnAttribute {
    /// CHANNEL-NUMBER attribute (2 bytes channel number + 2 bytes RFFU)
    ChannelNumber(u16),
    /// LIFETIME attribute (4 bytes, in seconds)
    Lifetime(u32),
    /// XOR-PEER-ADDRESS attribute
    XorPeerAddress(SocketAddr),
    /// DATA attribute (variable length)
    Data(Bytes),
    /// XOR-RELAYED-ADDRESS attribute
    XorRelayedAddress(SocketAddr),
    /// EVEN-PORT attribute (1 byte flag + 3 bytes RFFU)
    EvenPort { reserve_next: bool },
    /// REQUESTED-TRANSPORT attribute (1 byte protocol + 3 bytes RFFU)
    RequestedTransport(TransportProtocol),
    /// DONT-FRAGMENT attribute (no value)
    DontFragment,
    /// RESERVATION-TOKEN attribute (8 bytes)
    ReservationToken([u8; 8]),
    /// Unknown TURN attribute
    Unknown { attr_type: u16, value: Bytes },
}

/// Transport protocol for TURN
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportProtocol {
    /// UDP (17)
    Udp = 17,
    /// TCP (6)
    Tcp = 6,
}

impl TransportProtocol {
    /// Parse from protocol number
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            17 => Some(Self::Udp),
            6 => Some(Self::Tcp),
            _ => None,
        }
    }

    /// Convert to protocol number
    pub fn to_u8(self) -> u8 {
        self as u8
    }
}

impl TurnAttribute {
    /// Get attribute type code
    pub fn attr_type(&self) -> u16 {
        match self {
            Self::ChannelNumber(_) => TurnAttributeType::ChannelNumber as u16,
            Self::Lifetime(_) => TurnAttributeType::Lifetime as u16,
            Self::XorPeerAddress(_) => TurnAttributeType::XorPeerAddress as u16,
            Self::Data(_) => TurnAttributeType::Data as u16,
            Self::XorRelayedAddress(_) => TurnAttributeType::XorRelayedAddress as u16,
            Self::EvenPort { .. } => TurnAttributeType::EvenPort as u16,
            Self::RequestedTransport(_) => TurnAttributeType::RequestedTransport as u16,
            Self::DontFragment => TurnAttributeType::DontFragment as u16,
            Self::ReservationToken(_) => TurnAttributeType::ReservationToken as u16,
            Self::Unknown { attr_type, .. } => *attr_type,
        }
    }

    /// Encode attribute to bytes
    pub fn encode(&self, transaction_id: &[u8; 12]) -> Bytes {
        let mut buf = BytesMut::new();
        
        // Attribute type (2 bytes)
        buf.put_u16(self.attr_type());
        
        // Encode value
        let _value_start = buf.len() + 2; // Skip length field
        match self {
            Self::ChannelNumber(channel) => {
                buf.put_u16(4); // Length
                buf.put_u16(*channel);
                buf.put_u16(0); // RFFU
            }
            Self::Lifetime(lifetime) => {
                buf.put_u16(4); // Length
                buf.put_u32(*lifetime);
            }
            Self::XorPeerAddress(addr) => {
                let value = encode_xor_address(*addr, transaction_id);
                buf.put_u16(value.len() as u16);
                buf.put(value);
            }
            Self::Data(data) => {
                buf.put_u16(data.len() as u16);
                buf.put(data.clone());
            }
            Self::XorRelayedAddress(addr) => {
                let value = encode_xor_address(*addr, transaction_id);
                buf.put_u16(value.len() as u16);
                buf.put(value);
            }
            Self::EvenPort { reserve_next } => {
                buf.put_u16(4); // Length
                buf.put_u8(if *reserve_next { 0x80 } else { 0x00 });
                buf.put_u8(0); // RFFU
                buf.put_u16(0); // RFFU
            }
            Self::RequestedTransport(protocol) => {
                buf.put_u16(4); // Length
                buf.put_u8(protocol.to_u8());
                buf.put_u8(0); // RFFU
                buf.put_u16(0); // RFFU
            }
            Self::DontFragment => {
                buf.put_u16(0); // No value
            }
            Self::ReservationToken(token) => {
                buf.put_u16(8); // Length
                buf.put_slice(token);
            }
            Self::Unknown { value, .. } => {
                buf.put_u16(value.len() as u16);
                buf.put(value.clone());
            }
        }
        
        // Add padding to 4-byte boundary
        let padding = (4 - (buf.len() % 4)) % 4;
        for _ in 0..padding {
            buf.put_u8(0);
        }
        
        buf.freeze()
    }

    /// Decode attribute from bytes
    pub fn decode(attr_type: u16, value: Bytes, transaction_id: &[u8; 12]) -> Result<Self, String> {
        match attr_type {
            x if x == TurnAttributeType::ChannelNumber as u16 => {
                if value.len() < 4 {
                    return Err("ChannelNumber too short".to_string());
                }
                let mut buf = value;
                let channel = buf.get_u16();
                Ok(Self::ChannelNumber(channel))
            }
            x if x == TurnAttributeType::Lifetime as u16 => {
                if value.len() < 4 {
                    return Err("Lifetime too short".to_string());
                }
                let mut buf = value;
                let lifetime = buf.get_u32();
                Ok(Self::Lifetime(lifetime))
            }
            x if x == TurnAttributeType::XorPeerAddress as u16 => {
                let addr = decode_xor_address(&value, transaction_id)?;
                Ok(Self::XorPeerAddress(addr))
            }
            x if x == TurnAttributeType::Data as u16 => {
                Ok(Self::Data(value))
            }
            x if x == TurnAttributeType::XorRelayedAddress as u16 => {
                let addr = decode_xor_address(&value, transaction_id)?;
                Ok(Self::XorRelayedAddress(addr))
            }
            x if x == TurnAttributeType::EvenPort as u16 => {
                if value.len() < 4 {
                    return Err("EvenPort too short".to_string());
                }
                let mut buf = value;
                let flags = buf.get_u8();
                let reserve_next = (flags & 0x80) != 0;
                Ok(Self::EvenPort { reserve_next })
            }
            x if x == TurnAttributeType::RequestedTransport as u16 => {
                if value.len() < 4 {
                    return Err("RequestedTransport too short".to_string());
                }
                let mut buf = value;
                let protocol_num = buf.get_u8();
                let protocol = TransportProtocol::from_u8(protocol_num)
                    .ok_or_else(|| format!("Unknown transport protocol: {}", protocol_num))?;
                Ok(Self::RequestedTransport(protocol))
            }
            x if x == TurnAttributeType::DontFragment as u16 => {
                Ok(Self::DontFragment)
            }
            x if x == TurnAttributeType::ReservationToken as u16 => {
                if value.len() < 8 {
                    return Err("ReservationToken too short".to_string());
                }
                let mut token = [0u8; 8];
                token.copy_from_slice(&value[..8]);
                Ok(Self::ReservationToken(token))
            }
            _ => Ok(Self::Unknown { attr_type, value }),
        }
    }
}

/// Encode socket address with XOR obfuscation
fn encode_xor_address(addr: SocketAddr, transaction_id: &[u8; 12]) -> Bytes {
    let mut buf = BytesMut::new();
    
    // Reserved (1 byte) + Family (1 byte)
    buf.put_u8(0);
    match addr {
        SocketAddr::V4(_) => buf.put_u8(0x01), // IPv4
        SocketAddr::V6(_) => buf.put_u8(0x02), // IPv6
    }
    
    // XOR port with magic cookie (upper 16 bits)
    let xor_port = addr.port() ^ ((MAGIC_COOKIE >> 16) as u16);
    buf.put_u16(xor_port);
    
    // XOR address
    match addr.ip() {
        IpAddr::V4(ipv4) => {
            let octets = ipv4.octets();
            let magic_bytes = MAGIC_COOKIE.to_be_bytes();
            for i in 0..4 {
                buf.put_u8(octets[i] ^ magic_bytes[i]);
            }
        }
        IpAddr::V6(ipv6) => {
            let octets = ipv6.octets();
            let mut xor_key = Vec::with_capacity(16);
            xor_key.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
            xor_key.extend_from_slice(transaction_id);
            
            for i in 0..16 {
                buf.put_u8(octets[i] ^ xor_key[i]);
            }
        }
    }
    
    buf.freeze()
}

/// Decode XOR-obfuscated socket address
fn decode_xor_address(data: &Bytes, transaction_id: &[u8; 12]) -> Result<SocketAddr, String> {
    if data.len() < 4 {
        return Err("XOR address too short".to_string());
    }
    
    let mut buf = data.clone();
    buf.get_u8(); // Skip reserved
    let family = buf.get_u8();
    let xor_port = buf.get_u16();
    
    // Decode port
    let port = xor_port ^ ((MAGIC_COOKIE >> 16) as u16);
    
    // Decode address
    match family {
        0x01 => {
            // IPv4
            if buf.remaining() < 4 {
                return Err("IPv4 address too short".to_string());
            }
            let magic_bytes = MAGIC_COOKIE.to_be_bytes();
            let mut octets = [0u8; 4];
            for i in 0..4 {
                octets[i] = buf.get_u8() ^ magic_bytes[i];
            }
            Ok(SocketAddr::new(IpAddr::V4(Ipv4Addr::from(octets)), port))
        }
        0x02 => {
            // IPv6
            if buf.remaining() < 16 {
                return Err("IPv6 address too short".to_string());
            }
            let mut xor_key = Vec::with_capacity(16);
            xor_key.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
            xor_key.extend_from_slice(transaction_id);
            
            let mut octets = [0u8; 16];
            for i in 0..16 {
                octets[i] = buf.get_u8() ^ xor_key[i];
            }
            Ok(SocketAddr::new(IpAddr::V6(Ipv6Addr::from(octets)), port))
        }
        _ => Err(format!("Unknown address family: {}", family)),
    }
}

/// Lifetime helper
#[derive(Debug, Clone, Copy)]
pub struct Lifetime(pub u32);

impl Lifetime {
    /// Create from seconds
    pub fn from_secs(secs: u32) -> Self {
        Self(secs)
    }

    /// Create from Duration
    pub fn from_duration(duration: Duration) -> Self {
        Self(duration.as_secs() as u32)
    }

    /// Get as seconds
    pub fn as_secs(&self) -> u32 {
        self.0
    }

    /// Get as Duration
    pub fn as_duration(&self) -> Duration {
        Duration::from_secs(self.0 as u64)
    }
}

/// Requested transport helper
#[derive(Debug, Clone, Copy)]
pub struct RequestedTransport(pub TransportProtocol);

impl RequestedTransport {
    /// Create UDP transport
    pub fn udp() -> Self {
        Self(TransportProtocol::Udp)
    }

    /// Create TCP transport
    pub fn tcp() -> Self {
        Self(TransportProtocol::Tcp)
    }

    /// Get protocol
    pub fn protocol(&self) -> TransportProtocol {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_number_encode_decode() {
        let transaction_id = [0u8; 12];
        let attr = TurnAttribute::ChannelNumber(0x4000);
        let encoded = attr.encode(&transaction_id);
        
        // Skip type and length
        let value = encoded.slice(4..);
        let decoded = TurnAttribute::decode(
            TurnAttributeType::ChannelNumber as u16,
            value,
            &transaction_id
        ).unwrap();
        
        match decoded {
            TurnAttribute::ChannelNumber(channel) => assert_eq!(channel, 0x4000),
            _ => panic!("Wrong attribute type"),
        }
    }

    #[test]
    fn test_lifetime_encode_decode() {
        let transaction_id = [0u8; 12];
        let attr = TurnAttribute::Lifetime(600);
        let encoded = attr.encode(&transaction_id);
        
        let value = encoded.slice(4..);
        let decoded = TurnAttribute::decode(
            TurnAttributeType::Lifetime as u16,
            value,
            &transaction_id
        ).unwrap();
        
        match decoded {
            TurnAttribute::Lifetime(lifetime) => assert_eq!(lifetime, 600),
            _ => panic!("Wrong attribute type"),
        }
    }

    #[test]
    fn test_xor_peer_address_ipv4() {
        let transaction_id = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let addr = "192.168.1.100:8080".parse().unwrap();
        let attr = TurnAttribute::XorPeerAddress(addr);
        let encoded = attr.encode(&transaction_id);
        
        let value = encoded.slice(4..);
        let decoded = TurnAttribute::decode(
            TurnAttributeType::XorPeerAddress as u16,
            value,
            &transaction_id
        ).unwrap();
        
        match decoded {
            TurnAttribute::XorPeerAddress(decoded_addr) => assert_eq!(decoded_addr, addr),
            _ => panic!("Wrong attribute type"),
        }
    }

    #[test]
    fn test_requested_transport() {
        let transaction_id = [0u8; 12];
        let attr = TurnAttribute::RequestedTransport(TransportProtocol::Udp);
        let encoded = attr.encode(&transaction_id);
        
        let value = encoded.slice(4..);
        let decoded = TurnAttribute::decode(
            TurnAttributeType::RequestedTransport as u16,
            value,
            &transaction_id
        ).unwrap();
        
        match decoded {
            TurnAttribute::RequestedTransport(protocol) => {
                assert_eq!(protocol, TransportProtocol::Udp);
            }
            _ => panic!("Wrong attribute type"),
        }
    }

    #[test]
    fn test_data_attribute() {
        let transaction_id = [0u8; 12];
        let data = Bytes::from_static(b"Hello, TURN!");
        let attr = TurnAttribute::Data(data.clone());
        let encoded = attr.encode(&transaction_id);
        
        let value = encoded.slice(4..);
        let decoded = TurnAttribute::decode(
            TurnAttributeType::Data as u16,
            value,
            &transaction_id
        ).unwrap();
        
        match decoded {
            TurnAttribute::Data(decoded_data) => assert_eq!(decoded_data, data),
            _ => panic!("Wrong attribute type"),
        }
    }
}

// Made with Bob