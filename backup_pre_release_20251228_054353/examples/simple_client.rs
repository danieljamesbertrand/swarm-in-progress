//! Simple example showing QUIC transport configuration
//! 
//! This example demonstrates how to configure and use QUIC transport
//! for P2P communication in the Promethos-AI Swarm.

use punch_simple::quic_transport::{
    TransportType, 
    get_listen_address, 
    get_dual_listen_addresses,
};

fn main() {
    println!("=== Promethos-AI Swarm - Transport Configuration Example ===\n");

    // QUIC addresses
    println!("QUIC Transport:");
    let quic_addr = get_listen_address(TransportType::QuicOnly, 51820);
    println!("  Listen address: {}", quic_addr);
    
    // TCP addresses  
    println!("\nTCP Transport:");
    let tcp_addr = get_listen_address(TransportType::TcpOnly, 51820);
    println!("  Listen address: {}", tcp_addr);
    
    // Dual-stack addresses
    println!("\nDual-Stack Transport (QUIC preferred, TCP fallback):");
    let (quic, tcp) = get_dual_listen_addresses(51820);
    println!("  QUIC address: {}", quic);
    println!("  TCP address:  {}", tcp);
    
    // Parsing transport type from string
    println!("\nParsing transport types:");
    let types = ["quic", "tcp", "dual", "QUIC-only", "TCP-only", "both"];
    for t in types {
        match t.parse::<TransportType>() {
            Ok(tt) => println!("  '{}' -> {:?}", t, tt),
            Err(e) => println!("  '{}' -> Error: {}", t, e),
        }
    }
    
    println!("\n=== Configuration Complete ===");
    println!("\nTo run a node with QUIC transport:");
    println!("  listener --transport quic --port 51820");
    println!("\nTo run a node with dual-stack transport:");
    println!("  listener --transport dual --port 51820");
}
