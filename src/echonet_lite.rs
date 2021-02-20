// Reference: ECHONET-Lite_Ver.1.12_02.pdf
// https://echonet.jp/wp/wp-content/uploads/pdf/General/Standard/ECHONET_lite_V1_12_jp/ECHONET-Lite_Ver.1.12_02.pdf

use num_enum::{TryFromPrimitive, TryFromPrimitiveError};
use std::convert::TryFrom;

#[derive(Debug)]
pub struct Error {
    description: String,
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        f.write_str(&self.description)
    }
}

impl<T: TryFromPrimitive> From<TryFromPrimitiveError<T>> for Error {
    fn from(error: TryFromPrimitiveError<T>) -> Self {
        Self {
            description: format!("{:?}", error),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub type EHD1 = u8;
pub const ECHONET_LITE_HEADER1: EHD1 = 0x10;

#[derive(Clone, Copy, Debug, TryFromPrimitive)]
#[repr(u8)]
pub enum EHD2 {
    Format1 = 0x81,
    Format2 = 0x82,
}

pub type TID = u16;

#[derive(Debug)]
pub struct EOJ {
    /// class group code
    pub x1: u8,
    /// class code
    pub x2: u8,
    /// instance code
    pub x3: u8,
}

impl EOJ {
    fn as_bytes(&self) -> [u8; 3] {
        [self.x1, self.x2, self.x3]
    }
}

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, TryFromPrimitive)]
#[repr(u8)]
pub enum ESV {
    // Requests
    SetI = 0x60,
    SetC = 0x61,
    Get = 0x62,
    INF_REQ = 0x63,
    SetGet = 0x6E,
}

#[derive(Debug)]
pub struct EProp {
    /// echonet property code
    epc: u8,
    /// property data counter
    pdc: u8,
    /// echonet data
    edt: Vec<u8>,
}

impl EProp {
    fn as_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![self.epc, self.pdc];
        bytes.extend(self.edt.iter());
        bytes
    }
    fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            epc: bytes[0],
            pdc: bytes[1],
            edt: bytes[2..].to_vec(),
        }
    }
}

#[derive(Debug)]
pub enum EDATA {
    Format1 {
        /// sender object
        seoj: EOJ,
        /// dest object
        deoj: EOJ,
        /// echonet service
        esv: ESV,
        /// object property counter
        /// `props.len() == opc`
        opc: u8,
        props: Vec<EProp>,
    },
    Format2(Vec<u8>),
}

#[derive(Debug)]
pub struct EFrame {
    pub ehd1: EHD1,
    pub ehd2: EHD2,
    pub tid: TID,
    pub edata: EDATA,
}

impl EFrame {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let ehd2 = EHD2::try_from(bytes[1])?;
        let edata: EDATA;
        match ehd2 {
            EHD2::Format1 => {
                let opc = bytes[11];
                let mut props = vec![];
                let mut tail_cursor = 12;
                for i in 0..opc {
                    let epc = bytes[tail_cursor];
                    tail_cursor += 1;
                    let pdc = bytes[tail_cursor];
                    tail_cursor += 1;
                    let edt = bytes[tail_cursor..tail_cursor + pdc as usize].to_vec();
                    tail_cursor += pdc as usize;
                    props.push(EProp { epc, pdc, edt });
                }

                edata = EDATA::Format1 {
                    seoj: EOJ {
                        x1: bytes[4],
                        x2: bytes[5],
                        x3: bytes[6],
                    },
                    deoj: EOJ {
                        x1: bytes[7],
                        x2: bytes[8],
                        x3: bytes[9],
                    },
                    esv: ESV::try_from(bytes[10])?,
                    opc: opc,
                    props: props,
                }
            }
            EHD2::Format2 => {
                edata = EDATA::Format2(bytes[4..].into());
            }
        }
        Ok(Self {
            ehd1: bytes[0],
            ehd2: ehd2,
            tid: TID::from_be_bytes([bytes[2], bytes[3]]),
            edata: edata,
        })
    }
    pub fn as_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![self.ehd1, self.ehd2 as u8];
        bytes.extend_from_slice(&self.tid.to_be_bytes());
        match &self.edata {
            EDATA::Format1 {
                seoj,
                deoj,
                esv,
                opc,
                props,
            } => {
                bytes.extend_from_slice(&seoj.as_bytes());
                bytes.extend_from_slice(&deoj.as_bytes());
                bytes.push(*esv as u8);
                bytes.push(*opc);
                for prop in props {
                    bytes.extend(prop.as_bytes());
                }
            }
            EDATA::Format2(data) => {
                bytes.extend(data);
            }
        }
        bytes
    }
}
