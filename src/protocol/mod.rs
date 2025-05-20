use anyhow::Result;
use log::{info, error};

pub const CMD_READ: u8 = 0x0a;
pub const CMD_WRITE: u8 = 0x09;
pub const CMD_RESP: u8 = 0x0b;

#[derive(Debug)]
pub struct RequestHeader {
    pub control_bit: u8,
    pub total_len: u32,
    pub chip_addr: u8,
    pub data_len: u32,
    pub param_addr: u16,
}

#[derive(Debug)]
pub struct WriteHeader {
    pub control_bit: u8,
    pub safeload: u8,
    pub channel_num: u8,
    pub total_len: u32,
    pub chip_addr: u8,
    pub data_len: u32,
    pub param_addr: u16,
}

#[derive(Debug)]
pub struct ResponseHeader {
    pub control_bit: u8,
    pub total_len: u32,
    pub chip_addr: u8,
    pub data_len: u32,
    pub param_addr: u16,
    pub success: u8,
    pub reserved: [u8; 1],
}

impl RequestHeader {
    pub fn from_bytes(buf: &[u8]) -> Result<Self> {
        Ok(Self {
            control_bit: buf[0],
            total_len: u32::from_be_bytes(buf[1..5].try_into()?),
            chip_addr: buf[5],
            data_len: u32::from_be_bytes(buf[6..10].try_into()?),
            param_addr: u16::from_be_bytes(buf[10..12].try_into()?),
        })
    }
}

impl WriteHeader {
    pub fn from_bytes(buf: &[u8]) -> Result<Self> {
        if buf.len() < 14 {
            return Err(anyhow::anyhow!("Buffer too short for write header"));
        }
        Ok(Self {
            control_bit: buf[0],
            safeload: buf[1],
            channel_num: buf[2],
            total_len: u32::from_be_bytes([buf[3], buf[4], buf[5], buf[6]]),
            chip_addr: buf[7],
            data_len: u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]),
            param_addr: u16::from_be_bytes([buf[12], buf[13]]),
        })
    }
}

impl ResponseHeader {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(13);
        bytes.push(self.control_bit);
        bytes.extend_from_slice(&self.total_len.to_be_bytes());
        bytes.push(self.chip_addr);
        bytes.extend_from_slice(&self.data_len.to_be_bytes());
        bytes.extend_from_slice(&self.param_addr.to_be_bytes());
        bytes.push(self.success);
        bytes.extend_from_slice(&self.reserved);
        bytes
    }
}

#[derive(Debug)]
pub enum ProtocolCommand {
    Read {
        header: RequestHeader,
    },
    Write {
        header: WriteHeader,
        data: Vec<u8>,
    },
    Unknown(u8),
}

#[derive(Debug)]
pub enum ProtocolResponse {
    Read {
        header: ResponseHeader,
        data: Vec<u8>,
    },
    Write {
        header: ResponseHeader,
    },
    Error(String),
}

pub struct ProtocolHandler;

impl ProtocolHandler {
    pub fn parse_command(buf: &[u8]) -> Result<ProtocolCommand> {
        if buf.is_empty() {
            return Err(anyhow::anyhow!("Empty buffer"));
        }

        match buf[0] {
            CMD_READ => {
                if buf.len() >= 12 {
                    let header = RequestHeader::from_bytes(buf)?;
                    Ok(ProtocolCommand::Read { header })
                } else {
                    Err(anyhow::anyhow!("Buffer too short for read command"))
                }
            }
            CMD_WRITE => {
                if buf.len() >= 14 {
                    let header = WriteHeader::from_bytes(buf)?;
                    if buf.len() >= 14 + header.data_len as usize {
                        let data = buf[14..14 + header.data_len as usize].to_vec();
                        Ok(ProtocolCommand::Write { header, data })
                    } else {
                        Err(anyhow::anyhow!("Buffer too short for write data"))
                    }
                } else {
                    Err(anyhow::anyhow!("Buffer too short for write command"))
                }
            }
            cmd => Ok(ProtocolCommand::Unknown(cmd)),
        }
    }

    pub fn create_read_response(chip_addr: u8, data_len: u32, param_addr: u16, data: Vec<u8>) -> ProtocolResponse {
        let header = ResponseHeader {
            control_bit: CMD_RESP,
            total_len: 13 + data.len() as u32,
            chip_addr,
            data_len,
            param_addr,
            success: 0,
            reserved: [0],
        };
        ProtocolResponse::Read { header, data }
    }

    pub fn create_write_response(chip_addr: u8, data_len: u32, param_addr: u16) -> ProtocolResponse {
        let header = ResponseHeader {
            control_bit: CMD_RESP,
            total_len: 13,
            chip_addr,
            data_len,
            param_addr,
            success: 0,
            reserved: [0],
        };
        ProtocolResponse::Write { header }
    }

    pub fn create_error_response(error: String) -> ProtocolResponse {
        ProtocolResponse::Error(error)
    }
}





#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_command_f6fb_example() {
        // Example from program:
        // Read Request for IC 1, Param Address: 0xF6FB, Bytes: 2
        // rx [a, 0, 0, 0, e, 1, 0, 0, 0, 2, f6, fb, 0, 0]
        let buf = [
            0x0a, // CMD_READ
            0x00, 0x00, 0x00, 0x0e, // total_len = 14
            0x01, // chip_addr = 1 (IC 1)
            0x00, 0x00, 0x00, 0x02, // data_len = 2
            0xf6, 0xfb, // param_addr = 0xf6fb (SPDIF_TX_VB_RIGHT_11)
            0x00, 0x00  // trailing bytes
        ];
        let cmd = ProtocolHandler::parse_command(&buf).unwrap();
        
        match cmd {
            ProtocolCommand::Read { header } => {
                assert_eq!(header.control_bit, CMD_READ);
                assert_eq!(header.total_len, 0x0e);
                assert_eq!(header.chip_addr, 0x01);
                assert_eq!(header.data_len, 0x02);
                assert_eq!(header.param_addr, 0xf6fb);
            }
            _ => panic!("Expected Read command"),
        }
    }

    #[test]
    fn test_write_command_zero_total_len() {
        // Test case from logs: write command with total_len = 0
        // rx [9, 0, 0, 0, 0, 0, 10, 1, 0, 0, 0, 2, f0, 0, 0, 0]
        let buf = [
            0x09, // CMD_WRITE
            0x00, // safeload
            0x00, // channel_num
            0x00, 0x00, 0x00, 0x10, // total_len = 16
            0x01, // chip_addr = 1
            0x00, 0x00, 0x00, 0x02, // data_len = 2
            0xf0, 0x00, // param_addr = 0xf000
            0x00, 0x00  // data payload
        ];
        let cmd = ProtocolHandler::parse_command(&buf).unwrap();
        
        match cmd {
            ProtocolCommand::Write { header, data } => {
                assert_eq!(header.control_bit, CMD_WRITE);
                assert_eq!(header.safeload, 0x00);
                assert_eq!(header.channel_num, 0x00);
                assert_eq!(header.total_len, 0x10);
                assert_eq!(header.chip_addr, 0x01);
                assert_eq!(header.data_len, 0x02);
                assert_eq!(header.param_addr, 0xf000);
                assert_eq!(data, vec![0x00, 0x00]);
            }
            _ => panic!("Expected Write command"),
        }
    }

    #[test]
    fn test_write_command_f020_example() {
        // Example from program:
        // Block Write to IC 1, Param Address: 0xF020, Data: [0x00, 0x08] (2 bytes)
        // rx [9, 0, 0, 0, 0, 0, 10, 1, 0, 0, 0, 2, f0, 20, 0, 8]
        let buf = [
            0x09, // CMD_WRITE
            0x00, // safeload
            0x00, // channel_num
            0x00, 0x00, 0x00, 0x10, // total_len = 16
            0x01, // chip_addr = 1 (IC 1)
            0x00, 0x00, 0x00, 0x02, // data_len = 2
            0xf0, 0x20, // param_addr = 0xf020
            0x00, 0x08  // data payload: [0x00, 0x08]
        ];
        let cmd = ProtocolHandler::parse_command(&buf).unwrap();
        
        match cmd {
            ProtocolCommand::Write { header, data } => {
                assert_eq!(header.control_bit, CMD_WRITE);
                assert_eq!(header.safeload, 0x00);
                assert_eq!(header.channel_num, 0x00);
                assert_eq!(header.total_len, 0x10);
                assert_eq!(header.chip_addr, 0x01);
                assert_eq!(header.data_len, 0x02);
                assert_eq!(header.param_addr, 0xf020);
                assert_eq!(data, vec![0x00, 0x08]);
            }
            _ => panic!("Expected Write command"),
        }
    }

    #[test]
    fn test_write_command_large_zeros() {
        // Example from program:
        // Block Write to IC 1, Param Address: 0x0000 (DM0 Data), Bytes: 80 (all zeros)
        // [2025-05-20T19:16:28Z DEBUG sigma_tcp_rs] rx [9, 0, 0, 0, 0, 0, 5e, 1, 0, 0, 0, 50, 0, 0, ...]
        
        // Create our test buffer with 14-byte header + 80 bytes of zeros
        let mut buf = vec![
            0x09, // CMD_WRITE
            0x00, // safeload
            0x00, // channel_num
            0x00, 0x00, 0x00, 0x5e, // total_len = 94 (14 + 80)
            0x01, // chip_addr = 1 (IC 1)
            0x00, 0x00, 0x00, 0x50, // data_len = 80 (0x50)
            0x00, 0x00, // param_addr = 0x0000 (DM0 Data)
        ];
        
        // Add 80 bytes of zeros for the data payload
        buf.extend(vec![0x00; 80]);
        
        let cmd = ProtocolHandler::parse_command(&buf.as_slice()).unwrap();
        
        match cmd {
            ProtocolCommand::Write { header, data } => {
                assert_eq!(header.control_bit, CMD_WRITE);
                assert_eq!(header.safeload, 0x00);
                assert_eq!(header.channel_num, 0x00);
                assert_eq!(header.total_len, 0x5e);
                assert_eq!(header.chip_addr, 0x01);
                assert_eq!(header.data_len, 0x50);
                assert_eq!(header.param_addr, 0x0000);
                
                // Verify data is 80 zeros
                assert_eq!(data.len(), 80);
                assert!(data.iter().all(|&b| b == 0x00));
            }
            _ => panic!("Expected Write command"),
        }
    }

    #[test]
    fn test_read_command_sequence() {
        // Test sequence of read commands from logs
        let addresses = [0xf6f5, 0xf6f6, 0xf6f7, 0xf6f8, 0xf6f9, 0xf6fa, 0xf6fb];
        
        for addr in addresses {
            let mut buf = [0x0a, 0x00, 0x00, 0x00, 0x0e, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00];
            buf[10] = (addr >> 8) as u8;
            buf[11] = addr as u8;
            
            let cmd = ProtocolHandler::parse_command(&buf).unwrap();
            
            match cmd {
                ProtocolCommand::Read { header } => {
                    assert_eq!(header.control_bit, CMD_READ);
                    assert_eq!(header.total_len, 0x0e);
                    assert_eq!(header.chip_addr, 0x01);
                    assert_eq!(header.data_len, 0x02);
                    assert_eq!(header.param_addr, addr);
                }
                _ => panic!("Expected Read command"),
            }
        }
    }

    #[test]
    fn test_response_creation() {
        // Test response creation for both read and write
        let read_response = ProtocolHandler::create_read_response(0x01, 2, 0xf6f5, vec![0x00, 0x00]);
        match read_response {
            ProtocolResponse::Read { header, data } => {
                assert_eq!(header.control_bit, CMD_RESP);
                assert_eq!(header.total_len, 15); // 13 + data_len
                assert_eq!(header.chip_addr, 0x01);
                assert_eq!(header.data_len, 2);
                assert_eq!(header.param_addr, 0xf6f5);
                assert_eq!(data, vec![0x00, 0x00]);
            }
            _ => panic!("Expected Read response"),
        }

        let write_response = ProtocolHandler::create_write_response(0x01, 2, 0xf6f5);
        match write_response {
            ProtocolResponse::Write { header } => {
                assert_eq!(header.control_bit, CMD_RESP);
                assert_eq!(header.total_len, 13);
                assert_eq!(header.chip_addr, 0x01);
                assert_eq!(header.data_len, 2);
                assert_eq!(header.param_addr, 0xf6f5);
            }
            _ => panic!("Expected Write response"),
        }
    }
} 