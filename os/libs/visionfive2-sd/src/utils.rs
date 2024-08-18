use crate::register::SDIO_BASE;

pub fn read_fifo<T: SDIo>(io: &T, addr: usize) -> u64 {
    io.read_data_at(addr - SDIO_BASE)
}

pub fn write_fifo<T: SDIo>(io: &mut T, addr: usize, val: u64) {
    io.write_data_at(addr - SDIO_BASE, val);
}

pub fn write_reg<T: SDIo>(io: &mut T, addr: usize, val: u32) {
    io.write_reg_at(addr - SDIO_BASE, val);
}

pub fn read_reg<T: SDIo>(io: &T, addr: usize) -> u32 {
    io.read_reg_at(addr - SDIO_BASE)
}

pub trait SDIo {
    fn read_reg_at(&self, offset: usize) -> u32;
    fn write_reg_at(&mut self, offset: usize, val: u32);
    fn read_data_at(&self, offset: usize) -> u64;
    fn write_data_at(&mut self, offset: usize, val: u64);
}

pub trait SleepOps {
    fn sleep_ms(ms: usize);
    fn sleep_ms_until(ms: usize, f: impl FnMut() -> bool);
}

pub trait GetBit {
    type Output;
    fn get_bit(&self, bit: u8) -> bool;
    fn get_bits(&self, start: u8, end: u8) -> Self::Output;
}

impl GetBit for u32 {
    type Output = u32;
    fn get_bit(&self, bit: u8) -> bool {
        (*self & (1 << bit)) != 0
    }
    fn get_bits(&self, start: u8, end: u8) -> Self::Output {
        let mask = (1 << (end - start + 1)) - 1;
        (*self >> start) & mask
    }
}

impl GetBit for u128 {
    type Output = u128;
    fn get_bit(&self, bit: u8) -> bool {
        (*self & (1 << bit)) != 0
    }
    fn get_bits(&self, start: u8, end: u8) -> Self::Output {
        let mask = (1 << (end - start + 1)) - 1;
        (*self >> start) & mask
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    #[test]
    fn test_get_bit() {
        let val = 0b1010_1010u32;
        assert_eq!(val.get_bit(0), false);
        assert_eq!(val.get_bit(1), true);
        assert_eq!(val.get_bit(2), false);
        assert_eq!(val.get_bit(3), true);
        assert_eq!(val.get_bit(4), false);
        assert_eq!(val.get_bit(5), true);
        assert_eq!(val.get_bit(6), false);
        assert_eq!(val.get_bit(7), true);
    }

    #[test]
    fn test_get_bits() {
        let val = 0b1010_1010u32;
        assert_eq!(val.get_bits(0, 3), 0b1010);
        assert_eq!(val.get_bits(4, 7), 0b1010);
    }
}
