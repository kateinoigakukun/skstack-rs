use anyhow::Result;
use log::debug;
use skstack_rs::{SKEvent, SKPan, SKSTACK};
mod config;
mod echonet_lite;

fn main() -> Result<()> {
    env_logger::init();
    let mut skstack = crate::SKSTACK::open("/dev/tty.usbserial-DJ00QQY8".to_string())?;
    let version = skstack.version()?;
    println!("version: {}", version);
    skstack.set_password(config::ROUTEB_PASSWORD)?;
    skstack.set_rbid(config::ROUTEB_ID)?;

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
    skstack.join(ip_v6_addr.clone())?;

    let frame = echonet_lite::EFrame {
        ehd1: echonet_lite::ECHONET_LITE_HEADER1,
        ehd2: echonet_lite::EHD2::Format1,
        tid: 0x1234,
        edata: echonet_lite::EDATA::Format1 {
            seoj: echonet_lite::EOJ {
                x1: 0x05,
                x2: 0xff,
                x3: 0x01,
            },
            deoj: config::TARGET_EOJ,
            esv: echonet_lite::ESV::Get,
            opc: 1,
            props: vec![echonet_lite::EProp {
                epc: 0xE7,
                pdc: 0,
                edt: vec![],
            }],
        },
    };

    skstack.send_udp(1, 3610, ip_v6_addr, &frame.as_bytes())?;

    println!("start loop");
    loop {
        let event = skstack.read_event()?;
        match event {
            SKEvent::ERXUDP { data, .. } => {
                let frame = echonet_lite::EFrame::from_bytes(&data)?;
                println!("{:?}", frame);
            }
            other => {
                println!("other event: {:?}", other);
            }
        }
    }
    Ok(())
}
