//! LSB-first bit writer / reader. SPEC §3.1.

#[derive(Default)]
pub struct BitWriter {
    pub buf: Vec<u8>,
    acc: u64,
    nbits: u32,
}

impl BitWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn write(&mut self, value: u32, nbits: u32) {
        debug_assert!(nbits <= 32);
        let mask = if nbits == 32 { u32::MAX } else { (1u32 << nbits) - 1 };
        self.acc |= ((value & mask) as u64) << self.nbits;
        self.nbits += nbits;
        while self.nbits >= 8 {
            self.buf.push((self.acc & 0xff) as u8);
            self.acc >>= 8;
            self.nbits -= 8;
        }
    }

    pub fn finish(mut self) -> Vec<u8> {
        if self.nbits > 0 {
            self.buf.push((self.acc & 0xff) as u8);
        }
        self.buf
    }
}

pub struct BitReader<'a> {
    buf: &'a [u8],
    pos: usize,
    acc: u64,
    nbits: u32,
}

impl<'a> BitReader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0, acc: 0, nbits: 0 }
    }

    pub fn read(&mut self, nbits: u32) -> u32 {
        debug_assert!(nbits <= 32);
        while self.nbits < nbits {
            let byte = if self.pos < self.buf.len() {
                let b = self.buf[self.pos];
                self.pos += 1;
                b
            } else {
                0
            };
            self.acc |= (byte as u64) << self.nbits;
            self.nbits += 8;
        }
        let mask = if nbits == 32 { u32::MAX as u64 } else { (1u64 << nbits) - 1 };
        let v = (self.acc & mask) as u32;
        self.acc >>= nbits;
        self.nbits -= nbits;
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_bits() {
        let mut w = BitWriter::new();
        w.write(0b1010, 4);
        w.write(0b11, 2);
        w.write(0xff, 8);
        w.write(0x1234, 16);
        let bytes = w.finish();
        let mut r = BitReader::new(&bytes);
        assert_eq!(r.read(4), 0b1010);
        assert_eq!(r.read(2), 0b11);
        assert_eq!(r.read(8), 0xff);
        assert_eq!(r.read(16), 0x1234);
    }
}
