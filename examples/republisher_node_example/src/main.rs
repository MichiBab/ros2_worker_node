use anyhow::{Error, Result};
use rclrs::*;
mod demo_node;

fn main() -> Result<(), Error> {
    let context = Context::default_from_env()?;

    let mut executor = context.create_basic_executor();

    let node = executor.create_node("example_node")?;
    let _background_node_demo = demo_node::MyRosNodeExample::new_node(&node)?;
    println!("Background node created");

    executor.spin(SpinOptions::default());

    Ok(())
}
