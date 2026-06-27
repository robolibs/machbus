//! Parse a synthetic IOP buffer and emit the FNV-1a "version" string.
//! Mirrors `iop_parser_demo.cpp`.

use machbus::net::{hash_to_version, parse_iop_data, validate};

fn main() {
    println!("=== IOP Parser Demo ===");

    // Synthesize a tiny conformant ISO 11783-6 IOP buffer (no per-object
    // length prefix; boundaries recovered by the codec walker):
    //   - id=1, type=Container (3): w=100, h=200, hidden=0, no children/macros
    //   - id=2, type=NumberVariable (21): value=0xCAFEF00D
    let mut buf = Vec::new();
    buf.extend_from_slice(&[0x01, 0x00]); // id = 1
    buf.push(3); // type = Container
    buf.extend_from_slice(&[100, 0, 200, 0, 0]); // w=100, h=200, hidden=0
    buf.extend_from_slice(&[0, 0]); // num_objects=0, num_macros=0

    buf.extend_from_slice(&[0x02, 0x00]); // id = 2
    buf.push(21); // type = NumberVariable
    buf.extend_from_slice(&[0x0D, 0xF0, 0xFE, 0xCA]); // value LE

    println!("buffer size = {} bytes", buf.len());
    println!("validate    = {}", validate(&buf));

    let objs = parse_iop_data(&buf).unwrap();
    println!("parsed {} objects:", objs.len());
    for o in &objs {
        println!(
            "  id=0x{:04X}, type={:>2}, body_len={}",
            o.id,
            o.type_byte,
            o.body.len()
        );
    }

    println!("\nFNV-1a → 7-char version: {}", hash_to_version(&buf));
}
