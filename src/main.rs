use anyhow::Result;
use skstack_rs::{SKPan, SKSTACK};
mod config;

fn main() -> Result<()> {
    env_logger::init();
    let mut skstack = crate::SKSTACK::open("/dev/tty.usbserial-DJ00QQY8".to_string())?;
    let version = skstack.version()?;
    println!("version: {}", version);
    skstack.set_password(config::routeb_password)?;
    skstack.set_rbid(config::routeb_id)?;

    let mut duration = 6;
    let mut found: Vec<SKPan> = vec![];
    loop {
        println!("scanning (duration = {})", duration);
        found = skstack.scan(2, 0xFFFFFFFF, duration)?;
        if !found.is_empty() {
            break;
        }
        duration += 1;
        if duration > 15 {
            panic!("duration too long: {}", duration);
        }
    }
    println!("{:?}", found);
    Ok(())
}
