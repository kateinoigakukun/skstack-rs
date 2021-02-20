

use anyhow::Result;
use skstack_rs::SKSTACK;

fn main() -> Result<()> {
    let mut skstack = crate::SKSTACK::open("/dev/tty.usbserial-DJ00QQY8".to_string())?;
    let version = skstack.version()?;
    println!("version: {}", version);
    Ok(())
}
