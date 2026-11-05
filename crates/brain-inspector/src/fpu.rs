//! Explicit VFP instructions; the rest of Inspector retains its soft-float ABI.

pub const ROUNDS: u32 = 1_000_000;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Status {
    #[default]
    Idle,
    NoAccess,
    Unsupported,
    Pass,
    Fail,
}

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct Raw {
    pub fpexc: u32,
    pub fpscr: u32,
    pub mvfr0: u32,
    pub cpsr: u32,
    pub single: u32,
    pub fpscr_after: u32,
    pub double: u64,
    pub registers: [u64; 32],
}

#[derive(Clone, Copy, Default)]
pub struct Measurements {
    pub status: Status,
    pub cpacr: u32,
    pub raw: Raw,
    pub elapsed_us: u64,
    pub bad_registers: u32,
}

#[cfg(any(test, all(target_arch = "arm", feature = "device")))]
fn patterns() -> [u64; 32] {
    let mut values = core::array::from_fn(|i| 0x3ff0_1234_5678_0000 | i as u64);
    values[..4].copy_from_slice(&[
        1.5f64.to_bits(),
        2.25f64.to_bits(),
        2.0f64.to_bits(),
        3.0f64.to_bits(),
    ]);
    values
}

#[cfg(any(test, all(target_arch = "arm", feature = "device")))]
impl Measurements {
    fn check(&mut self) {
        let mut expected = patterns();
        expected[4] = 2.5f64.to_bits();
        self.bad_registers = self
            .raw
            .registers
            .iter()
            .zip(expected)
            .enumerate()
            .fold(0, |mask, (i, (&actual, expected))| {
                mask | (u32::from(actual != expected) << i)
            });
        self.status = if self.raw.single == 2.5f32.to_bits()
            && self.raw.double == 2.5f64.to_bits()
            && self.raw.fpscr_after == 0
            && self.bad_registers == 0
        {
            Status::Pass
        } else {
            Status::Fail
        };
    }
}

#[cfg(all(target_arch = "arm", feature = "device"))]
core::arch::global_asm!(
    include_str!("fpu.S"),
    fpexc = const core::mem::offset_of!(Raw, fpexc),
    fpscr = const core::mem::offset_of!(Raw, fpscr),
    mvfr0 = const core::mem::offset_of!(Raw, mvfr0),
    cpsr = const core::mem::offset_of!(Raw, cpsr),
    single = const core::mem::offset_of!(Raw, single),
    fpscr_after = const core::mem::offset_of!(Raw, fpscr_after),
    double = const core::mem::offset_of!(Raw, double),
    registers = const core::mem::offset_of!(Raw, registers),
);

/// Runs hardware arithmetic without changing the application's soft-float ABI.
///
/// # Safety
/// Requires privileged execution on PW-H1, including its timestamp counter.
#[cfg(all(target_arch = "arm", feature = "device"))]
pub unsafe fn run() -> Measurements {
    unsafe extern "C" {
        fn inspector_fpu_probe(input: *const u64, output: *mut Raw, rounds: u32);
    }
    let mut result = Measurements::default();
    unsafe {
        core::arch::asm!("mrc p15, 0, {value}, c1, c0, 2", value = out(reg) result.cpacr, options(nostack, preserves_flags));
    }
    // CP10/11 must permit privileged access, and D16-D31 must not be disabled.
    if result.cpacr & 0x0050_0000 != 0x0050_0000 || result.cpacr & (1 << 30) != 0 {
        result.status = Status::NoAccess;
        return result;
    }
    let values = patterns();
    unsafe {
        let start = reflex::runtime::micros();
        inspector_fpu_probe(values.as_ptr(), &mut result.raw, ROUNDS);
        result.elapsed_us = reflex::runtime::micros().wrapping_sub(start);
    }
    if result.raw.mvfr0 & 15 != 2 {
        result.status = Status::Unsupported;
    } else {
        result.check();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_arithmetic_and_every_register() {
        let mut result = Measurements::default();
        result.raw.single = 2.5f32.to_bits();
        result.raw.double = 2.5f64.to_bits();
        result.raw.registers = patterns();
        result.raw.registers[4] = result.raw.double;
        result.check();
        assert!(result.status == Status::Pass);
        for i in 0..32 {
            result.raw.registers[i] ^= 1;
            result.check();
            assert!(result.status == Status::Fail);
            assert_eq!(result.bad_registers, 1 << i);
            result.raw.registers[i] ^= 1;
        }
        result.raw.single ^= 1;
        result.check();
        assert!(result.status == Status::Fail);
        result.raw.single ^= 1;
        result.raw.double ^= 1;
        result.check();
        assert!(result.status == Status::Fail);
        result.raw.double ^= 1;
        result.raw.fpscr_after = 1;
        result.check();
        assert!(result.status == Status::Fail);
    }
}
