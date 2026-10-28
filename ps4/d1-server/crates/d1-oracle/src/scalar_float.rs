// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Opt-in arithmetic for the owned investment-curve paths. The measured Windows Unicorn 2.1.5
//! treats VEX scalar operations as destructive: with XMM0=2/XMM1=10,
//! `vaddss xmm0,xmm1,xmm0` returns 4 instead of 12. Interpret the four measured
//! register-register scalar operations only in the supplied ranges. This changes
//! neither image bytes nor a live client; ordinary codec harnesses are unaffected.

use crate::error::{OracleError, Result};
use std::{cell::RefCell, rc::Rc};
use unicorn_engine::{RegisterX86 as R, Unicorn};

pub(crate) struct ScalarFloat {
    fault: Rc<RefCell<Option<String>>>,
}

impl ScalarFloat {
    pub fn install(uc: &mut Unicorn<'static, ()>, ranges: &[(u64, u64)]) -> Result<Self> {
        let fault = Rc::new(RefCell::new(None));
        let error = fault.clone();
        let ranges = ranges.to_vec();
        uc.add_code_hook(1, 0, move |uc, address, _| {
            if !ranges.iter().any(|(lo, hi)| (*lo..*hi).contains(&address)) {
                return;
            }
            let result = (|| -> std::result::Result<(), String> {
                let b = uc
                    .mem_read_as_vec(address, 4)
                    .map_err(|e| format!("{e:?}"))?;
                // VEX2, scalar single precision, L=0, ADD/MUL/SUB/DIV.
                if b[0] != 0xC5 || b[1] & 7 != 2 || ![0x58, 0x59, 0x5C, 0x5E].contains(&b[2]) {
                    return Ok(());
                }
                let destination = ((b[3] >> 3) & 7) | (((!b[1] >> 7) & 1) << 3);
                let first = (!b[1] >> 3) & 15;
                if b[3] & 0xC0 != 0xC0 {
                    // Same-source memory forms already have destructive semantics.
                    return if first == destination {
                        Ok(())
                    } else {
                        Err(format!("unsupported scalar memory form at {address:#x}"))
                    };
                }
                let second = b[3] & 7;
                // Read both operands before writing; destination may alias either source.
                let x = uc
                    .reg_read_long(R::XMM0 as i32 + first as i32)
                    .map_err(|e| format!("{e:?}"))?;
                let y = uc
                    .reg_read_long(R::XMM0 as i32 + second as i32)
                    .map_err(|e| format!("{e:?}"))?;
                let a = f32::from_le_bytes(x[..4].try_into().unwrap());
                let v = f32::from_le_bytes(y[..4].try_into().unwrap());
                let value = match b[2] {
                    0x58 => a + v,
                    0x59 => a * v,
                    0x5C => a - v,
                    0x5E => a / v,
                    _ => unreachable!(),
                };
                if !a.is_finite()
                    || !v.is_finite()
                    || !value.is_finite()
                    || a.is_subnormal()
                    || v.is_subnormal()
                    || value.is_subnormal()
                {
                    return Err(format!("unsupported scalar value at {address:#x}"));
                }
                // VEX scalar forms copy source1 bits127:32 and zero bits255:128.
                let mut out = [0u8; 32];
                out[..16].copy_from_slice(&x);
                out[..4].copy_from_slice(&value.to_le_bytes());
                uc.reg_write_long(R::YMM0 as i32 + destination as i32, &out)
                    .map_err(|e| format!("{e:?}"))?;
                uc.reg_write(R::RIP, address + 4)
                    .map_err(|e| format!("{e:?}"))?;
                Ok(())
            })();
            if let Err(message) = result {
                *error.borrow_mut() = Some(message);
                let _ = uc.emu_stop();
            }
        })
        .map_err(|e| OracleError::Unicorn(format!("{e:?}")))?;
        Ok(Self { fault })
    }

    pub fn check(&self) -> Result<()> {
        match self.fault.borrow().as_ref() {
            Some(message) => Err(OracleError::Fault(message.clone())),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicorn_engine::{Arch, Mode, Prot};

    fn engine(code: &[u8]) -> Unicorn<'static, ()> {
        let mut uc = Unicorn::new(Arch::X86, Mode::MODE_64).unwrap();
        uc.mem_map(0x1000, 0x1000, Prot::ALL).unwrap();
        uc.mem_write(0x1000, code).unwrap();
        for (r, value) in [
            (R::YMM0, 2f32),
            (R::YMM1, 10f32),
            (R::YMM2, 0.25f32),
            (R::YMM3, 0f32),
        ] {
            let mut bytes = [0x71u8; 32];
            bytes[..4].copy_from_slice(&value.to_le_bytes());
            uc.reg_write_long(r, &bytes).unwrap();
        }
        uc
    }
    fn low(uc: &Unicorn<'static, ()>, r: R) -> f32 {
        f32::from_le_bytes(uc.reg_read_long(r).unwrap()[..4].try_into().unwrap())
    }

    #[test]
    fn aliases_and_all_four_operations_preserve_upper_source_and_zero_upper_destination() {
        for (code, destination, expected) in [
            ([0xC5, 0xF2, 0x58, 0xC0], R::YMM0, 12f32),
            ([0xC5, 0xF2, 0x58, 0xD8], R::YMM3, 12f32),
            ([0xC5, 0xF2, 0x5C, 0xC0], R::YMM0, 8f32),
            ([0xC5, 0xF2, 0x59, 0xCA], R::YMM1, 2.5f32),
            ([0xC5, 0xF2, 0x5E, 0xC0], R::YMM0, 5f32),
        ] {
            let mut uc = engine(&code);
            let shim = ScalarFloat::install(&mut uc, &[(0x1000, 0x1004)]).unwrap();
            uc.emu_start(0x1000, 0x1004, 0, 1).unwrap();
            shim.check().unwrap();
            assert_eq!(low(&uc, destination), expected);
            let bytes = uc.reg_read_long(destination).unwrap();
            assert_eq!(&bytes[4..16], &[0x71; 12]);
            assert_eq!(&bytes[16..], &[0; 16]);
        }
    }
    #[test]
    fn unmeasured_memory_operands_fail_closed() {
        let mut uc = engine(&[0xC5, 0xF2, 0x58, 0x00]);
        let shim = ScalarFloat::install(&mut uc, &[(0x1000, 0x1004)]).unwrap();
        uc.emu_start(0x1000, 0x1004, 0, 1).unwrap();
        assert!(shim
            .check()
            .unwrap_err()
            .to_string()
            .contains("memory form"));
    }
    #[test]
    fn no_hook_effect_outside_opted_in_range() {
        let mut uc = engine(&[0xC5, 0xFA, 0x58, 0xC1]);
        let shim = ScalarFloat::install(&mut uc, &[(0x2000, 0x2004)]).unwrap();
        uc.emu_start(0x1000, 0x1004, 0, 1).unwrap();
        shim.check().unwrap();
        assert_eq!(low(&uc, R::YMM0), 12f32);
    }

    #[test]
    fn non_finite_results_are_refused() {
        let mut uc = engine(&[0xC5, 0xF2, 0x5E, 0xC3]); // xmm0=xmm1/xmm3 (zero)
        let shim = ScalarFloat::install(&mut uc, &[(0x1000, 0x1004)]).unwrap();
        uc.emu_start(0x1000, 0x1004, 0, 1).unwrap();
        assert!(shim.check().is_err());
    }
}
