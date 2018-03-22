extern crate pnet;

use pnet::packet::udp::MutableUdpPacket;
use pnet::packet::ipv4::MutableIpv4Packet;
use pnet::packet::MutablePacket;

fn main() {
    for interface in pnet::datalink::interfaces() {
        println!("{}", interface);
    }
    let mut packet_buffer = vec![0u8; 60];
    {
        let mut udp_packet = MutableUdpPacket::new(&mut packet_buffer[20..]).unwrap();
        udp_packet.set_destination(53u16);
        udp_packet.set_payload(&[61u8, 62u8, 63u8]);
        udp_packet.set_length(3);
    }
    {
        let mut ip_packet = MutableIpv4Packet::new(&mut packet_buffer).unwrap();
        ip_packet.set_total_length(100);
    }
    println!("Hello, packet: {:?}!", packet_buffer);
}
