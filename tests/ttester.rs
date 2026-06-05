use ferth::io::BufIo;
use ferth::{Fe, Result};

#[test]
fn test_load_test_harness() -> Result<()> {
    let src: Vec<u8> = [
        include_bytes!("ttester-shim.fth") as &[u8],
        include_bytes!("ttester.4th"),
    ]
    .concat();
    let mut dest: Vec<u8> = vec![];
    let io = BufIo::new(&src, &mut dest);
    let mut fe = Fe::new([0u8; 65536], io)?;
    fe.evaluate(b"quit")
}
