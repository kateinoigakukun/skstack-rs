use std::time::Duration;

use anyhow::Result;
use log::{debug, error, warn};
use nix::unistd::sleep;
use rand::Rng;
use skstack_rs::echonet_lite;
use skstack_rs::skstack::{SKEvent, SKPan, SKSTACK};

const TARGET_EOJ: echonet_lite::EOJ = echonet_lite::EOJ {
    /// 住宅・設備関連機器クラスグループ
    x1: 0x02,
    /// 低圧スマート電力量メータ
    x2: 0x88,
    x3: 0x01,
};

fn main() -> Result<()> {
    env_logger::init();
    let device_path = std::env::var("DEVICE_PATH")?;
    let routeb_password = std::env::var("ROUTEB_PASSWORD")?;
    let routeb_id = std::env::var("ROUTEB_ID")?;

    let mut skstack = crate::SKSTACK::open(device_path, None)?;
    let version = skstack.version()?;
    println!("version: {}", version);
    skstack.set_password(routeb_password)?;
    skstack.set_rbid(routeb_id)?;

    let mut duration = 4;
    let mut found: Vec<SKPan>;
    loop {
        debug!("scanning (duration = {})", duration);
        found = skstack.scan(2, 0xFFFFFFFF, duration)?;
        if !found.is_empty() {
            break;
        }
        duration += 1;
        if duration > 15 {
            panic!("duration too long: {}", duration);
        }
    }
    let found = found.first().unwrap();
    debug!("found PAN: {:?}", found);
    skstack.set_register("S2", format!("{:X}", found.channel))?;
    skstack.set_register("S3", format!("{:X}", found.pan_id))?;
    let ip_v6_addr = skstack.get_link_local_addr(found.addr.clone())?;
    skstack.join(&ip_v6_addr)?;

    skstack.set_timeout(Some(Duration::from_millis(10000)));
    let mut rng = rand::thread_rng();
    'main_loop: loop {
        let tid = rng.gen();
        let frame = echonet_lite::EFrame {
            ehd1: echonet_lite::ECHONET_LITE_HEADER1,
            ehd2: echonet_lite::EHD2::Format1,
            tid: tid,
            edata: echonet_lite::EDATA::Format1 {
                seoj: echonet_lite::EOJ {
                    x1: 0x05,
                    x2: 0xff,
                    x3: 0x01,
                },
                deoj: TARGET_EOJ,
                esv: echonet_lite::ESV::Get,
                opc: 1,
                props: vec![echonet_lite::EProp {
                    /// 瞬時電力計測値
                    epc: 0xE7,
                    pdc: 0,
                    edt: vec![],
                }],
            },
        };
        skstack.send_udp(1, 3610, &ip_v6_addr, &frame.as_bytes())?;

        loop {
            let event = match skstack.read_event() {
                Ok(event) => event,
                Err(error) => {
                    if error.is_timeout() {
                        warn!("timedout: {:?}", error);
                        continue 'main_loop;
                    }
                    error!("{:?}", error);
                    return Err(error.into());
                }
            };
            match event {
                SKEvent::ERXUDP { data, .. } => {
                    let frame = echonet_lite::EFrame::from_bytes(&data).expect("read frame");
                    debug!("{:?}", frame);
                    if frame.tid != tid {
                        continue;
                    }
                    let value = match frame.edata {
                        echonet_lite::EDATA::Format1 { props, .. } => {
                            let prop = props.first().unwrap();
                            assert_eq!(prop.edt.len(), 4);
                            let mut bytes: [u8; 4] = Default::default();
                            bytes.copy_from_slice(&prop.edt);
                            i32::from_be_bytes(bytes)
                        }
                        echonet_lite::EDATA::Format2 { .. } => {
                            panic!("unexpected format2 response!")
                        }
                    };
                    println!("⚡ {}w", value);
                    sleep(1);
                    break;
                }
                _ => {}
            }
        }
    }
}
