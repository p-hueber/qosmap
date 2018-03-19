extern crate pnet;

fn main() {
    for interface in pnet::datalink::interfaces() {
        println!("{}", interface);
    }
    println!("Hello, world!");
}
