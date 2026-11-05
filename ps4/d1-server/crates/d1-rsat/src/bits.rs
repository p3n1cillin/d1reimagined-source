// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The client's **bitstream writer**, modelled exactly.
//!
//! Read from build 56578, where the writer sits at the same addresses as 01.29:
//!
//! | function | what it does |
//! |---|---|
//! | inline (every leaf encoder, `0x1028590`..`0x10286ed`) | if `64 - n >= w`: `acc = (acc << w) \| v; n += w; bits += w` |
//! | `0x733A0` (32-bit spill) / `0x73000` (64-bit write) | otherwise: `room = 64 - n; n' = w - room`; emit `n' <= 63 ? (acc << room) \| (v >> n') : acc` as 8 big-endian bytes; `acc = v; n = n'` |
//! | `0x1028550` (bool) / presence bits | the same, one bit, with the whole source byte as `v` |
//! | `0x72560` (flush) | `acc << (64 - n)`, emitted big-endian; the reader takes `ceil(bits / 8)` bytes |
//!
//! The value is **never masked to its width**. A leaf whose value overflows its width ORs the extra
//! bits into the bits already in the accumulator, exactly as the client does. Well-formed images
//! never do that; synthetic equivalence tests do, so the model keeps it rather than hiding it. Bits
//! that would land in an already-emitted word are lost, as in the client.
//!
//! The result is MSB-first, continuous across fields, with the final byte zero-padded: the same
//! convention as `d1_bap`'s `pack_msb_first` and the sensor framing's writer. Those are clean
//! writers for values that fit; this one exists because byte identity with the client's encoder,
//! on any image, needs the accumulator's behaviour, not just its convention.

/// One bitstream being written, as the client's `bs+0x24/+0x28/+0x30` triple.
#[derive(Debug, Clone, Default)]
pub struct BitWriter {
    acc: u64,
    /// Bits currently held in `acc` (`bs+0x30`), 0..=64.
    held: u32,
    /// Total bits written (`bs+0x24`).
    bits: u64,
    out: Vec<u8>,
}

impl BitWriter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Bits written so far.
    pub fn bit_len(&self) -> u64 {
        self.bits
    }

    /// Write `width` bits of `value` (0..=64), unmasked, as the client does.
    pub fn write(&mut self, value: u64, width: u32) {
        debug_assert!(width <= 64);
        let room = 64 - self.held;
        self.bits += u64::from(width);
        if room >= width {
            self.acc = if width <= 63 { self.acc << width } else { 0 } | value;
            self.held += width;
            return;
        }
        let spill = width - room;
        let word = if spill <= 63 { (self.acc << room) | (value >> spill) } else { self.acc };
        self.out.extend_from_slice(&word.to_be_bytes());
        self.acc = value;
        self.held = spill;
    }

    /// One bit taken from a source byte, as the bool encoder and the walker's presence bits do.
    pub fn write_bit_byte(&mut self, byte: u8) {
        self.write(u64::from(byte), 1);
    }

    pub fn write_bit(&mut self, bit: bool) {
        self.write(u64::from(bit), 1);
    }

    /// Append the first `nbits` bits of an MSB-first payload (another encoder's output).
    pub fn append(&mut self, payload: &[u8], nbits: u64) {
        let mut done = 0u64;
        while done < nbits {
            let take = (nbits - done).min(32) as u32;
            let mut v = 0u64;
            for k in 0..u64::from(take) {
                let i = done + k;
                let bit = (payload[(i >> 3) as usize] >> (7 - (i & 7))) & 1;
                v = (v << 1) | u64::from(bit);
            }
            self.write(v, take);
            done += u64::from(take);
        }
    }

    /// Flush (`0x72560`) and return `(bits, ceil(bits / 8) bytes)`.
    pub fn finish(mut self) -> (u64, Vec<u8>) {
        if self.held > 0 {
            let word = if self.held == 64 { self.acc } else { self.acc << (64 - self.held) };
            self.out.extend_from_slice(&word.to_be_bytes());
        }
        let n = self.bits.div_ceil(8) as usize;
        self.out.resize(n, 0);
        (self.bits, self.out)
    }
}

/// The client's bitstream **reader** (`0x72C70` for up to 32 bits, `0x73170` for 64, `0x73430` for
/// one), for the widths a well-formed schema uses: the next `width` bits, MSB-first.
///
/// The client reads zeros past the end of the buffer and its callers then reject the decode because
/// the bit position passed the length (`0x2A4AC0`'s `bits <= len * 8`, the oracle's
/// `ShortPayload`). This reader refuses the read itself; the outcome is the same refusal.
///
/// Width 0 is not modelled: `0x72C70` then returns the cached word whole (`shr` by 64 is a shift by
/// 0 on x86), which depends on the reader's cache alignment. [`crate::Schemas::check_decode`]
/// refuses such leaves statically.
#[derive(Debug, Clone)]
pub struct BitReader<'a> {
    buf: &'a [u8],
    pos: u64,
}

impl<'a> BitReader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    /// A reader starting at bit `pos`.
    pub fn at(buf: &'a [u8], pos: u64) -> Self {
        Self { buf, pos }
    }

    /// Bits consumed so far (`bs+0x24`).
    pub fn bit_pos(&self) -> u64 {
        self.pos
    }

    /// Bits the buffer holds.
    pub fn bit_len(&self) -> u64 {
        self.buf.len() as u64 * 8
    }

    /// Read `width` (1..=64) bits; `None` past the end.
    pub fn read(&mut self, width: u32) -> Option<u64> {
        debug_assert!((1..=64).contains(&width));
        if self.pos + u64::from(width) > self.bit_len() {
            return None;
        }
        let v = read_bits(self.buf, self.pos, width);
        self.pos += u64::from(width);
        Some(v)
    }

    pub fn read_bit(&mut self) -> Option<bool> {
        self.read(1).map(|b| b == 1)
    }
}

/// Read `width` bits (<= 64) at bit `at` of an MSB-first payload. For tests and framing.
pub fn read_bits(payload: &[u8], at: u64, width: u32) -> u64 {
    let mut v = 0u64;
    for k in 0..u64::from(width) {
        let i = at + k;
        let bit = payload.get((i >> 3) as usize).map_or(0, |b| (b >> (7 - (i & 7))) & 1);
        v = (v << 1) | u64::from(bit);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits_of(w: BitWriter) -> String {
        let (n, b) = w.finish();
        (0..n).map(|i| if read_bits(&b, i, 1) == 1 { '1' } else { '0' }).collect()
    }

    #[test]
    fn values_that_fit_are_msb_first_and_continuous() {
        let mut w = BitWriter::new();
        w.write(0b101, 3);
        w.write(0xABCD, 16);
        w.write_bit(true);
        assert_eq!(bits_of(w), format!("101{:016b}1", 0xABCDu32));
    }

    #[test]
    fn the_final_byte_is_zero_padded() {
        let mut w = BitWriter::new();
        w.write(1, 1);
        let (n, b) = w.finish();
        assert_eq!((n, b), (1, vec![0x80]));
    }

    #[test]
    fn a_write_straddling_the_64_bit_word_spills_exactly() {
        for first in 1..64u32 {
            let mut w = BitWriter::new();
            let a = (1u64 << first) - 1; // all ones
            w.write(a, first);
            w.write(0x1234_5678_9ABC, 48);
            w.write(0b10, 2);
            let s = bits_of(w);
            let want = format!("{}{:048b}10", "1".repeat(first as usize), 0x1234_5678_9ABCu64);
            assert_eq!(s, want, "first width {first}");
        }
    }

    #[test]
    fn sixty_four_bit_writes_at_every_alignment() {
        for lead in 0..64u32 {
            let mut w = BitWriter::new();
            w.write(0, lead);
            w.write(0x8000_0000_0000_0001, 64);
            let s = bits_of(w);
            assert_eq!(s, format!("{}1{}1", "0".repeat(lead as usize), "0".repeat(62)), "{lead}");
        }
    }

    #[test]
    fn an_overflowing_value_pollutes_the_held_bits_like_the_client() {
        // acc = (acc << 2) | 0b111: the extra bit lands on the previous field's last bit.
        let mut w = BitWriter::new();
        w.write(0b10, 2);
        w.write(0b111, 2);
        assert_eq!(bits_of(w), "1111");
    }

    #[test]
    fn the_reader_reads_what_the_writer_wrote_and_refuses_past_the_end() {
        let mut w = BitWriter::new();
        w.write(0b101, 3);
        w.write(0xDEAD_BEEF_0123_4567, 64);
        w.write(1, 1);
        let (n, b) = w.finish();
        let mut r = BitReader::new(&b);
        assert_eq!(r.read(3), Some(0b101));
        assert_eq!(r.read(64), Some(0xDEAD_BEEF_0123_4567));
        assert_eq!(r.read_bit(), Some(true));
        assert_eq!(r.bit_pos(), n);
        // The final byte's padding is readable (the client checks bits <= len * 8), past it is not.
        assert_eq!(r.read(4), Some(0));
        assert_eq!(r.read(1), None);
    }

    #[test]
    fn append_reproduces_a_payload() {
        let mut a = BitWriter::new();
        for i in 0..77u64 {
            a.write(i & 1, 1);
        }
        let (n, p) = a.finish();
        let mut b = BitWriter::new();
        b.write(0b1, 1);
        b.append(&p, n);
        assert_eq!(bits_of(b)[1..], (0..77).map(|i| if i & 1 == 1 { '1' } else { '0' }).collect::<String>());
    }
}
