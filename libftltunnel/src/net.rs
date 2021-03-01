use std::io::Cursor;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use crc::crc32;

use crate::{FTLTunnelError, MAX_PACKET_SIZE};

const FRAME_HEAD_LEN: u16 = 11;

#[repr(C)]
// #[derive(Debug, Serialize, Deserialize)]
#[derive(Debug)]
pub struct NetFrame {
    frame_type: u8,
    /// This field is omitted from the final frame
    frame_size: u16,
    body_size: u16,
    flags: u16,
    /// The frame checksum is calculated wit this field set to 0xFFFFFFFF
    frame_checksum: u32,
    frame_body: Vec<u8>,
}

impl NetFrame {
    pub fn new(frame_type: u8, body_size: u16) -> Result<Self, crate::FTLTunnelError> {
        if 9 + body_size > MAX_PACKET_SIZE {
            return Err(FTLTunnelError::ChunkSizeTooLarge {
                allowed: MAX_PACKET_SIZE,
                got: (FRAME_HEAD_LEN + body_size) as _,
            });
        }

        let frame = NetFrame {
            frame_type,
            frame_size: (FRAME_HEAD_LEN + body_size),
            body_size,
            flags: 0,
            frame_checksum: 0xFFFFFFFF,
            frame_body: vec![0; body_size as _],
        };

        Ok(frame)
    }

    /// # Safety
    /// This function uses memory manipulation to output the struct as it's bytes
    /// it's only called when all memory it uses is initialized
    fn to_bytes(&self) -> Result<Vec<u8>, FTLTunnelError> {
        let mut buff = Vec::with_capacity(self.frame_size as _);
        buff.write_u8(self.frame_type)?;
        buff.write_u16::<BigEndian>(self.body_size)?;
        buff.write_u16::<BigEndian>(self.flags)?;
        buff.write_u32::<BigEndian>(self.frame_checksum)?;

        unsafe {
            std::ptr::copy(self.frame_body.as_ptr(),
                           buff.as_mut_ptr().offset(9),
                           self.frame_body.len());
            buff.set_len(self.frame_size as _);
        }
        Ok(buff)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, crate::FTLTunnelError> {
        let mut inner = vec![0; bytes.len()];
        inner.copy_from_slice(bytes);
        let mut caret = Cursor::new(inner);
        let frame_type = caret.read_u8()?;
        let body_size = caret.read_u16::<BigEndian>()?;
        let flags = caret.read_u16::<BigEndian>()?;
        let frame_checksum = caret.read_u32::<BigEndian>()?;

        if body_size + FRAME_HEAD_LEN > MAX_PACKET_SIZE {
            return Err(FTLTunnelError::InvalidPacket);
        }

        let mut frame = NetFrame {
            frame_type,
            frame_size: (FRAME_HEAD_LEN + body_size),
            body_size,
            flags,
            frame_checksum: 0xFFFFFFFF,
            frame_body: vec![0; body_size as _],
        };

        unsafe {
            std::ptr::copy(bytes.as_ptr().offset(9),
                           frame.frame_body.as_mut_ptr(),
                           frame.body_size as _);
            frame.frame_body.set_len(frame.body_size as _);
        };


        caret.write_u32::<BigEndian>(0xFFFFFFFF)?;

        if crc32::checksum_ieee(&*frame.to_bytes()?) != frame_checksum {
            return Err(FTLTunnelError::InvalidChecksum);
        }

        Ok(frame)
    }

    pub fn fill_body(&mut self, frame_body: &[u8]) -> Result<(), crate::FTLTunnelError> {
        if frame_body.len() > self.body_size as _ {
            return Err(FTLTunnelError::ChunkSizeTooLarge {
                allowed: self.body_size,
                got: frame_body.len(),
            });
        }

        if frame_body.len() < self.body_size as _ {
            for i in 0..frame_body.len() {
                self.frame_body[i] = frame_body[i];
            }
        } else {
            self.frame_body.copy_from_slice(frame_body);
        }
        Ok(())
    }

    pub fn checksum(&mut self) -> Result<(), crate::FTLTunnelError> {
        let serialized = self.to_bytes()?;
        self.frame_checksum = crc32::checksum_ieee(serialized.as_slice());
        Ok(())
    }

    pub fn encode(&mut self) -> Result<Vec<u8>, crate::FTLTunnelError> {
        if self.frame_checksum == 0xFFFFFFFF {
            self.checksum()?;
        }

        Ok(self.to_bytes()?)
    }
}
