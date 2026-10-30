// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! ⭐ The **vendor sale table**: what each vendor sells and what it costs, read from content.
//!
//! A purchase arrives as investment 901 (`0x80801777` = `{u8 vendor, i16 sale, u64}`), and the
//! client names nothing else: the server has to know which item sale `N` of vendor `V` is and what
//! it costs. That is content, so it is generated here from the owned packages, like
//! [`crate::item_index`]:
//!
//! ```text
//! d1-oracle vendor-sales write     # capture-rig/builds/<build>/vendor-sales.tsv
//! ```
//!
//! # Layout `[C]` content · `[O]` live (sense69, 30 Oct 2026)
//!
//! - The vendor table (tag [`VENDOR_TABLE_TAG`]): `u64 count @+0x20`, then `count` rows of 24 bytes
//!   from `+0x30`: `{u32 vendor hash @+0; u32 vendor definition tag @+0x10}`. The row index is the
//!   901 request's vendor (row 6 = the Guardian Outfitter, vendor hash `0x080760B4`).
//! - A vendor definition: the sale list `{u64 count @+0x28; u64 rel @+0x30}` → list header
//!   (`{u64 count, u32 class}`) at `0x30 + rel`, elements `0x70` bytes from header `+0x10`. The
//!   element index is the 901 request's sale index.
//! - A sale element: the item's content index `u16 @+0x3C`; the cost list `{u64 count @+0x18;
//!   u64 rel @+0x20}` → header at `+0x20 + rel`, cost elements 8 bytes from header `+0x10`:
//!   `{u32 quantity @+0; u16 content index @+4}` (a two-cost sale of Xûr's: 30 of 872 and 25 of 6189).
//!
//! Measured: the Outfitter's sale 23 is Command (content 840) for 570 Glimmer (content 869), and
//! sale 24 Crypto Shift (841) for 570. Those are the two purchases the client sent as
//! `{6, 23, …}` and `{6, 24, …}`. The vendor screen's price check (`0x26D6E0`, called at `0xA23FA7`)
//! reads the cost's content index from the same record (`+0xC` from the record the client holds).

use crate::error::{OracleError, Result};

/// The vendor table's content tag (`investment_globals` vendor lookup `0x58AB80`).
pub const VENDOR_TABLE_TAG: u32 = 0x80A5_FFCF;
/// Row stride of the vendor table.
pub const VENDOR_ROW: usize = 24;
/// Stride of a sale element.
pub const SALE_ELEMENT: usize = 0x70;
/// Stride of a cost element.
pub const COST_ELEMENT: usize = 8;
/// Default output, relative to the d1preservation tree.
pub const DEFAULT_PATH: &str = "capture-rig/builds/later-56578/vendor-sales.tsv";

/// One row of the vendor table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vendor {
    pub index: usize,
    pub hash: u32,
    pub tag: u32,
}

/// One item a vendor sells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sale {
    pub index: usize,
    pub content_index: u16,
    /// `(content index, quantity)` of each cost.
    pub costs: Vec<(u16, u32)>,
}

fn u64_at(b: &[u8], o: usize) -> Result<u64> {
    b.get(o..o + 8)
        .map(|s| u64::from_le_bytes(s.try_into().unwrap()))
        .ok_or_else(|| OracleError::Shape(format!("read past the end at {o:#x} ({} bytes)", b.len())))
}

fn u32_at(b: &[u8], o: usize) -> Result<u32> {
    b.get(o..o + 4)
        .map(|s| u32::from_le_bytes(s.try_into().unwrap()))
        .ok_or_else(|| OracleError::Shape(format!("read past the end at {o:#x} ({} bytes)", b.len())))
}

fn u16_at(b: &[u8], o: usize) -> Result<u16> {
    b.get(o..o + 2)
        .map(|s| u16::from_le_bytes(s.try_into().unwrap()))
        .ok_or_else(|| OracleError::Shape(format!("read past the end at {o:#x} ({} bytes)", b.len())))
}

/// `{u64 count @field; u64 rel @field+8}` → (count, first element offset). The list header sits at
/// `field + 8 + rel` and carries the same count; elements start 0x10 after it.
fn list(b: &[u8], field: usize) -> Result<(usize, usize)> {
    let count = u64_at(b, field)? as usize;
    if count == 0 {
        return Ok((0, 0));
    }
    let rel = u64_at(b, field + 8)? as usize;
    let header = field + 8 + rel;
    let again = u64_at(b, header)? as usize;
    if again != count {
        return Err(OracleError::Shape(format!(
            "list at {field:#x}: count {count} but its header at {header:#x} says {again}"
        )));
    }
    Ok((count, header + 0x10))
}

/// Parse the vendor table entry.
pub fn parse_vendor_table(b: &[u8]) -> Result<Vec<Vendor>> {
    let count = u64_at(b, 0x20)? as usize;
    if 0x30 + count * VENDOR_ROW > b.len() {
        return Err(OracleError::Shape(format!("{count} vendor rows do not fit in {} bytes", b.len())));
    }
    (0..count)
        .map(|i| {
            let r = 0x30 + i * VENDOR_ROW;
            Ok(Vendor { index: i, hash: u32_at(b, r)?, tag: u32_at(b, r + 0x10)? })
        })
        .collect()
}

/// Parse one vendor definition's sale list.
pub fn parse_vendor(b: &[u8]) -> Result<Vec<Sale>> {
    let (count, first) = list(b, 0x28)?;
    (0..count)
        .map(|i| {
            let e = first + i * SALE_ELEMENT;
            let content_index = u16_at(b, e + 0x3C)?;
            let (n, c0) = list(b, e + 0x18)?;
            let costs = (0..n)
                .map(|k| {
                    let c = c0 + k * COST_ELEMENT;
                    Ok((u16_at(b, c + 4)?, u32_at(b, c)?))
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(Sale { index: i, content_index, costs })
        })
        .collect()
}

/// Read every vendor's sales out of the packages.
pub fn read(content: &mut crate::oodle::Content) -> Result<Vec<(Vendor, Vec<Sale>)>> {
    let (_, _, _, table) = content.entry_bytes_for_tag(VENDOR_TABLE_TAG)?;
    let mut out = Vec::new();
    for v in parse_vendor_table(&table)? {
        if v.tag == 0 || v.tag == 0xFFFF_FFFF {
            out.push((v, Vec::new()));
            continue;
        }
        let (_, _, _, def) = content.entry_bytes_for_tag(v.tag)?;
        let sales = parse_vendor(&def).map_err(|e| {
            OracleError::Shape(format!("vendor {} (tag {:#010x}): {e}", v.index, v.tag))
        })?;
        out.push((v, sales));
    }
    Ok(out)
}

/// The TSV: one row per sale, `vendor sale vendor_hash content_index costs`, costs as
/// `content:quantity` joined with `,` (`-` for none).
pub fn render_tsv(vendors: &[(Vendor, Vec<Sale>)], generator: &str) -> String {
    let mut s = String::new();
    s.push_str("# vendor sales: what each vendor's sale N is and costs. The 901 request names {vendor, sale}.\n");
    s.push_str(&format!("# GENERATED by {generator} - do not hand-edit.\n"));
    s.push_str("vendor\tsale\tvendor_hash\tcontent_index\tcosts\n");
    for (v, sales) in vendors {
        for sale in sales {
            let costs = if sale.costs.is_empty() {
                "-".to_string()
            } else {
                sale.costs.iter().map(|(c, q)| format!("{c}:{q}")).collect::<Vec<_>>().join(",")
            };
            s.push_str(&format!(
                "{}\t{}\t{:#010x}\t{}\t{}\n",
                v.index, sale.index, v.hash, sale.content_index, costs
            ));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A synthetic vendor definition with the measured layout: two sales, the second with two costs.
    fn synthetic_vendor() -> Vec<u8> {
        let mut b = vec![0u8; 0x400];
        let put64 = |b: &mut Vec<u8>, o: usize, v: u64| b[o..o + 8].copy_from_slice(&v.to_le_bytes());
        // sale list: count 2 @0x28, rel @0x30 -> header at 0x100
        put64(&mut b, 0x28, 2);
        put64(&mut b, 0x30, 0x100 - 0x30);
        put64(&mut b, 0x100, 2);
        let first = 0x110;
        let costs_at = [0x300usize, 0x340];
        let ncost = [1u64, 2];
        for i in 0..2 {
            let e = first + i * SALE_ELEMENT;
            b[e + 0x3C..e + 0x3E].copy_from_slice(&(840u16 + i as u16).to_le_bytes());
            put64(&mut b, e + 0x18, ncost[i]);
            put64(&mut b, e + 0x20, (costs_at[i] - (e + 0x20)) as u64);
            put64(&mut b, costs_at[i], ncost[i]);
            for k in 0..ncost[i] as usize {
                let c = costs_at[i] + 0x10 + k * COST_ELEMENT;
                b[c..c + 4].copy_from_slice(&(570u32 + k as u32).to_le_bytes());
                b[c + 4..c + 6].copy_from_slice(&(869u16 + k as u16).to_le_bytes());
            }
        }
        b
    }

    #[test]
    fn a_vendor_definition_parses_to_its_sales_and_costs() {
        let sales = parse_vendor(&synthetic_vendor()).unwrap();
        assert_eq!(sales.len(), 2);
        assert_eq!(sales[0], Sale { index: 0, content_index: 840, costs: vec![(869, 570)] });
        assert_eq!(sales[1].content_index, 841);
        assert_eq!(sales[1].costs, vec![(869, 570), (870, 571)]);
    }

    #[test]
    fn a_header_that_disagrees_with_its_count_is_refused() {
        let mut b = synthetic_vendor();
        b[0x100..0x108].copy_from_slice(&3u64.to_le_bytes());
        assert!(parse_vendor(&b).is_err());
    }

    #[test]
    fn the_tsv_names_vendor_sale_item_and_costs() {
        let v = Vendor { index: 6, hash: 0x0807_60B4, tag: 0x80A6_61E9 };
        let s = vec![Sale { index: 23, content_index: 840, costs: vec![(869, 570)] }];
        let t = render_tsv(&[(v, s)], "test");
        assert!(t.contains("6\t23\t0x080760b4\t840\t869:570\n"), "{t}");
    }
}
