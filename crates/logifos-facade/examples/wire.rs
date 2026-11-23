//! A bounded native wire consumer for comparing transport envelopes.
use std::io::{self, Read, Write};

fn main() -> io::Result<()> {
    let mut input = Vec::new();
    io::stdin()
        .take((logifos_facade::MAX_REQUEST_BYTES + 1) as u64)
        .read_to_end(&mut input)?;
    io::stdout().write_all(&logifos_facade::request_bytes(&input))
}
