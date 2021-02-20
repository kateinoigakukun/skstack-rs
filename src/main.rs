use anyhow::Result;
use skstack_rs::{SKPan, SKSTACK};
use log::debug;
mod config;

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
    skstack.join(ip_v6_addr)?;
    Ok(())
}
