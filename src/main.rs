

use anyhow::Result;
use skstack_rs::SKSTACK;
mod config;

fn main() -> Result<()> {
    env_logger::init();
    let mut skstack = crate::SKSTACK::open("/dev/tty.usbserial-DJ00QQY8".to_string())?;
    let version = skstack.version()?;
    println!("version: {}", version);
    skstack.set_password(config::routeb_password)?;
    skstack.set_rbid(config::routeb_id)?;
    Ok(())
}
