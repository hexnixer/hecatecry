// Copyright 2021 Matheus Xavier
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::net::{SocketAddr, UdpSocket};
use orion::aead;

use fs3::FileExt;
use thiserror::Error;
use crate::TransactionStatus::{Uninitiated, Handshake};
use serde::{Serialize, Deserialize};
use rmp_serde::{Serializer, Deserializer};
use std::path::PathBuf;
use std::mem::size_of_val;
use crate::net::NetFrame;
use std::time::Duration;
use pretty_hex::PrettyHex;
use bytes::Bytes;

mod fs_layer;
mod net;
mod proto;

const MAX_PACKET_SIZE: u16 = 16_384;

/// Describes every possible error that can happen with an FTLTunnel
#[derive(Error, Debug)]
pub enum FTLTunnelError {
    #[error("IO Error")]
    IOError(#[from] std::io::Error),
    #[error("Serialization Error")]
    SerError,
    #[error("Failed to bind to socket")]
    BindFail { inner: std::io::Error },
    #[error("failed to de-serialize a dataframe")]
    InvalidPacket,
    #[error("failed to validate a dataframe")]
    InvalidChecksum,
    #[error("Chunk too big")]
    ChunkSizeTooLarge { allowed: u16, got: usize },
    #[error("Missing data for handshake")]
    MissingData,
}

#[derive(Debug)]
pub enum FailType {
    FailedToRingPeer,
    FailedToBindPort,
    FailedToPreallocate,
    LossTooHigh,
    NetworkError,
}

#[derive(Debug)]
pub enum TransactionStatus {
    Uninitiated,
    Handshake,
    Connected,
    Sending,
    Receiving,
    Failed { fail_type: FailType },
}

pub struct Transaction {
    peer_address: SocketAddr,
    listen_at: SocketAddr,
    sock: Option<UdpSocket>,
    target: Option<OsString>,
    target_size: Option<u64>,
    chunk_size: u16,
    // size is in multiples of target_size
    pre_buffer_size: u32,
    sender: bool,
    /// loss threshold in percent from 0 to 100 (recommended is 20)
    loss_threshold: u8,
    transaction_secret: Vec<u8>,
    transaction_status: TransactionStatus,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct HandshakeMessage {
    transaction_type: u8,
    transaction_filename: OsString,
    transaction_filesize: u64,
    chunk_size: u16,
    loss_max: u8,
}


fn send(sock: &UdpSocket, data: &[u8], dest: &SocketAddr) -> Result<usize, FTLTunnelError> {
    Ok(sock.send_to(data, dest)?)
}

fn recv(sock: &UdpSocket, data: &[u8], dest: &SocketAddr) -> Result<Bytes, FTLTunnelError> {
    let mut data = Vec::new();
    sock.recv_from()
}

impl Transaction {
    pub fn new(
        peer_address: SocketAddr,
        listen_at: SocketAddr,
        chunk_size: u16,
        buffer_ctn: u32,
        loss_threshold: u8,
        transaction_secret: &[u8],
    ) -> Result<Self, FTLTunnelError> {
        let key_normalized = blake3::hash(transaction_secret);

        let tran = Transaction {
            peer_address,
            listen_at,
            sock: None,
            target: None,
            target_size: None,
            chunk_size,
            pre_buffer_size: buffer_ctn,
            loss_threshold,
            sender: false,
            transaction_status: Uninitiated,
            transaction_secret: Vec::from(&key_normalized.as_bytes()[..]),
        };
        Ok(tran)
    }

    pub fn try_bind(&mut self) -> Result<(), FTLTunnelError> {
        let sock = match UdpSocket::bind(self.listen_at) {
            Ok(sock) => sock,
            Err(e) => return Err(FTLTunnelError::BindFail { inner: e }),
        };
        sock.set_read_timeout(Some(Duration::from_millis(3000)))?;
        self.sock = Some(sock);
        Ok(())
    }

    pub fn offer_send(&mut self, target: &PathBuf) -> Result<(), FTLTunnelError> {
        self.transaction_status = Handshake;
        self.target_size = Some(File::open(target)?.allocated_size()?);
        self.target = Some(OsString::from(target));
        self.try_bind()?;
        let target = self.target.as_ref().unwrap();
        let handshake_data = HandshakeMessage {
            transaction_type: 0x00,
            transaction_filename: target.clone(),
            transaction_filesize: self.target_size.unwrap(),
            chunk_size: self.chunk_size,
            loss_max: self.loss_threshold,
        };

        let mut buff = Vec::new();
        let mut frame = NetFrame::new(0, size_of_val(&handshake_data) as _)?;

        handshake_data.serialize(&mut Serializer::new(&mut buff)).unwrap();
        frame.fill_body(buff.as_slice())?;
        let data = frame.encode()?;
        send(self.sock.as_ref().unwrap(), data.as_slice(), &self.peer_address)?;
        let mut response = Vec::new();
        loop {
            if self.sock.as_ref().unwrap().recv(response.as_mut_slice())? != 0 {
                println!("{:?}", response.hex_dump());
                break;
            }
        }
        Ok(())
    }
}
