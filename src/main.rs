mod components;
mod network;
mod test;
mod client_plugin;
mod server_plugin;

use bevy::prelude::*;
use std::io;

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let default_address = "127.0.0.1:4444".to_string();
    let remote_address = args.get(1).unwrap_or(&default_address);
    
    let mut app = App::new();
    
    
    
    app.run();

    Ok(())
}
