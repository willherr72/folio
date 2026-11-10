//! Public synthetic data only; verify the envelope with an independent DPAPI caller.
use folio_engine::private_storage::{seal, Purpose};
fn main() {
    let output = std::path::PathBuf::from(std::env::args_os().nth(1).expect("output path"));
    let bytes = seal(Purpose::Source, b"Folio synthetic DPAPI interoperability probe").unwrap();
    std::fs::write(output, bytes).unwrap();
}
