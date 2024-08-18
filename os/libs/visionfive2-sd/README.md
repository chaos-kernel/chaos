# starfive2 SD card driver

This is a simple SD card driver for the StarFive2 board. 


## Usage
```rust
fn main(){
    pub fn sleep_ms(ms: usize) {
        let start = read_time();
        while read_time() - start < ms * VF2_FREQ / 1000 {
            core::hint::spin_loop();
        }
    }


    pub fn sleep_ms_until(ms: usize, mut f: impl FnMut() -> bool) {
        let start = read_time();
        while read_time() - start < ms * VF2_FREQ / 1000 {
            if f() {
                return;
            }
            core::hint::spin_loop();
        }
    }
    pub struct SdIoImpl;
    pub const SDIO_BASE: usize = 0x16020000;
    impl SDIo for SdIoImpl {
        fn read_data_at(&self, offset: usize) -> u64 {
            let addr = (SDIO_BASE + offset) as *mut u64;
            unsafe { addr.read_volatile() }
        }
        fn read_reg_at(&self, offset: usize) -> u32 {
            let addr = (SDIO_BASE + offset) as *mut u32;
            unsafe { addr.read_volatile() }
        }
        fn write_data_at(&mut self, offset: usize, val: u64) {
            let addr = (SDIO_BASE + offset) as *mut u64;
            unsafe { addr.write_volatile(val) }
        }
        fn write_reg_at(&mut self, offset: usize, val: u32) {
            let addr = (SDIO_BASE + offset) as *mut u32;
            unsafe { addr.write_volatile(val) }
        }
    }

    pub struct SleepOpsImpl;

    impl SleepOps for SleepOpsImpl {
        fn sleep_ms(ms: usize) {
            sleep_ms(ms)
        }
        fn sleep_ms_until(ms: usize, f: impl FnMut() -> bool) {
            sleep_ms_until(ms, f)
        }
    }
    let mut sd = Vf2SdDriver::<_, SleepOpsImpl>::new(SdIoImpl);
    sd.init();
    let mut buf = [0; 512];
    sd.read_block(0, &mut buf);
    println!("buf: {:x?}", &buf[..16]);
}
```


