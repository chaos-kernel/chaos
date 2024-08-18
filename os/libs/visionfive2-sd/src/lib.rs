#![no_std]
#[cfg(feature = "alloc")]
extern crate alloc;

use crate::cmd::*;
use crate::register::*;
use crate::utils::*;
use core::fmt::{Display, Formatter};
use core::mem::size_of;
use log::*;
use preprint::pprintln;

pub use utils::{SDIo, SleepOps};

mod cmd;
mod register;
mod utils;

enum DataTransType<'a> {
    None,
    Read(&'a mut [u8]),
    Write(&'a [u8]),
}

fn wait_ms_util_can_send_cmd<T: SDIo, S: SleepOps>(io: &mut T) -> bool {
    let f = || {
        let cmd_reg = CmdReg::from(read_reg(io, CMD_REG));
        !cmd_reg.start_cmd()
    };
    S::sleep_ms_until(1, f);
    f()
}

fn wait_ms_util_can_send_data<T: SDIo, S: SleepOps>(io: &mut T) -> bool {
    let f = || {
        let status_reg = StatusReg::from(read_reg(io, STATUS_REG));
        !status_reg.data_busy()
    };
    S::sleep_ms_until(100, f);
    f()
}

fn wait_ms_util_response<T: SDIo, S: SleepOps>(io: &mut T) -> bool {
    let f = || {
        let raw_int_status_reg = RawInterruptStatusReg::from(read_reg(io, RAW_INT_STATUS_REG));
        let int = raw_int_status_reg.int_status();
        let raw_int_status = RawInterrupt::from(int);
        raw_int_status.command_done()
    };
    S::sleep_ms_until(1, f);
    f()
}

fn fifo_filled_cnt<T: SDIo>(io: &mut T) -> usize {
    let status = StatusReg::from(read_reg(io, STATUS_REG));
    status.fifo_count() as usize
}

fn send_cmd<T: SDIo, S: SleepOps>(
    io: &mut T,
    cmd_type: Cmd,
    cmd: CmdReg,
    arg: CmdArg,
    data_trans_type: DataTransType,
) -> Option<[u32; 4]> {
    let res = wait_ms_util_can_send_cmd::<_, S>(io);
    assert!(res);
    if cmd.data_expected() {
        let res = wait_ms_util_can_send_data::<_, S>(io);
        assert!(res)
    }
    // info!("send cmd type:{:?}, value:{:#?}", cmd_type, cmd);
    // write arg
    write_reg(io, ARG_REG, arg.into());
    write_reg(io, CMD_REG, cmd.into());
    // Wait for cmd accepted
    let command_accept = wait_ms_util_can_send_cmd::<_, S>(io);
    // info!("command accepted {}", command_accept);

    if cmd.response_expect() {
        let res = wait_ms_util_response::<_, S>(io);
        // debug!("wait_ms_util_response:{:?}", res);
    }

    if cmd.data_expected() {
        let mut fifo_addr = FIFO_DATA_REG;
        match data_trans_type {
            DataTransType::Read(buffer) => {
                // trace!("data_expected read....");
                let mut buf_offset = 0;
                S::sleep_ms_until(250, || {
                    let raw_int_status_reg =
                        RawInterruptStatusReg::from(read_reg(io, RAW_INT_STATUS_REG));
                    let int = raw_int_status_reg.int_status();
                    let mut raw_int_status = RawInterrupt::from(int);
                    if raw_int_status.rxdr() {
                        // debug!("RXDR....");
                        while fifo_filled_cnt(io) >= 2 {
                            let data = read_fifo(io, fifo_addr);
                            for i in 0..8 {
                                buffer[buf_offset] = (data >> (i * 8)) as u8;
                                buf_offset += 1;
                            }
                            fifo_addr += size_of::<u64>();
                        }
                    }
                    raw_int_status.dto() || raw_int_status.have_error()
                });
                // info!(
                //     "buf_offset:{}, receive {} bytes",
                //     buf_offset,
                //     buf_offset * 8
                // );
            }
            DataTransType::Write(buffer) => {
                let mut buf_offset = 0;
                S::sleep_ms_until(250, || {
                    let raw_int_status = read_reg(io, RAW_INT_STATUS_REG);
                    let mut raw_int_status = RawInterrupt::from(raw_int_status as u16);
                    if raw_int_status.txdr() {
                        // debug!("TXDR....");
                        // Hard coded FIFO depth
                        while fifo_filled_cnt(io) < 120 && buf_offset < buffer.len() {
                            let mut data: u64 = 0;
                            for i in 0..8 {
                                data |= (buffer[buf_offset] as u64) << (i * 8);
                                buf_offset += 1;
                            }
                            write_fifo(io, fifo_addr, data);
                            fifo_addr += size_of::<u64>();
                        }
                    }
                    raw_int_status.dto() || raw_int_status.have_error()
                });
                // info!("buf_offset:{}, send {} bytes", buf_offset, buf_offset * 8);
            }
            _ => {
                panic!("Not implemented")
            }
        }
        // debug!("Current FIFO count: {}", fifo_filled_cnt(io));
    }
    // Clear interrupt by writing 1
    let raw_int_status = read_reg(io, RAW_INT_STATUS_REG);
    write_reg(io, RAW_INT_STATUS_REG, raw_int_status);
    // check error
    let raw_int_status = RawInterruptStatusReg::from(raw_int_status);
    let mut raw_int_status = RawInterrupt::from(raw_int_status.int_status());
    let resp = [
        read_reg(io, RESP0_REG),
        read_reg(io, RESP1_REG),
        read_reg(io, RESP2_REG),
        read_reg(io, RESP3_REG),
    ];
    if raw_int_status.have_error() {
        error!("card has error {:#?}", raw_int_status);
        error!("cmd {:#?}", cmd);
        error!("resp {:x?}", resp[0]);
        return None;
    }
    Some(resp)
}

fn reset_clock<T: SDIo, S: SleepOps>(io: &mut T) {
    // disable clock
    let mut clock_enable = ClockEnableReg::from(0);
    // write to CLOCK_ENABLE_REG
    write_reg(io, CLOCK_ENABLE_REG, clock_enable.into());
    // send reset clock command
    let clock_cmd = CmdReg::from(0)
        .with_start_cmd(true)
        .with_wait_prvdata_complete(true)
        .with_update_clock_registers_only(true);
    send_cmd::<_, S>(
        io,
        Cmd::ResetClock,
        clock_cmd,
        CmdArg::new(0),
        DataTransType::None,
    );
    // set clock divider to 400kHz (low)
    let clock_divider = ClockDividerReg::new().with_clk_divider0(4);
    write_reg(io, CLK_DIVIDER_REG, clock_divider.into());
    // send_cmd(Cmd::ResetClock,clock_disable_cmd,CmdArg::new(0));
    // enable clock
    clock_enable.set_clk_enable(1);
    write_reg(io, CLOCK_ENABLE_REG, clock_enable.into());
    // send reset clock command
    send_cmd::<_, S>(
        io,
        Cmd::ResetClock,
        clock_cmd,
        CmdArg::new(0),
        DataTransType::None,
    );
    // info!(
    //     "now clk enable {:#?}",
    //     ClockEnableReg::from(read_reg(io, CLOCK_ENABLE_REG))
    // );
    // pprintln!("reset clock success");
}

fn reset_fifo<T: SDIo>(io: &mut T) {
    let ctrl = ControlReg::from(read_reg(io, CTRL_REG)).with_fifo_reset(true);
    // todo!(why write to fifo data)?
    // write_reg(CTRL_REG,ctrl.raw());
    write_reg(io, FIFO_DATA_REG, ctrl.into());
    // pprintln!("reset fifo success");
}

fn reset_dma<T: SDIo>(io: &mut T) {
    let buf_mode_reg = BusModeReg::from(read_reg(io, BUS_MODE_REG))
        .with_de(false)
        .with_swr(true);
    write_reg(io, BUS_MODE_REG, buf_mode_reg.into());
    let ctrl = ControlReg::from(read_reg(io, CTRL_REG))
        .with_dma_reset(true)
        .with_use_internal_dmac(false);
    // ctrl.dma_enable().set(u1!(0));
    write_reg(io, CTRL_REG, ctrl.into());
    // pprintln!("reset dma success");
}

fn set_transaction_size<T: SDIo>(io: &mut T, blk_size: u32, byte_count: u32) {
    let blk_size = BlkSizeReg::new(blk_size);
    write_reg(io, BLK_SIZE_REG, blk_size.into());
    let byte_count = ByteCountReg::new(byte_count);
    write_reg(io, BYTE_CNT_REG, byte_count.into());
}

fn test_read<T: SDIo, S: SleepOps>(io: &mut T) {
    // pprintln!("test read, try read 0 block");
    set_transaction_size(io, 512, 512);
    let cmd17 = CmdReg::from(Cmd::ReadSingleBlock);
    let arg = CmdArg::new(0);
    let mut buffer: [u8; 512] = [0; 512];
    let _resp = send_cmd::<_, S>(
        io,
        Cmd::ReadSingleBlock,
        cmd17,
        arg,
        DataTransType::Read(&mut buffer),
    )
    .unwrap();
    // info!("Current FIFO count: {}", fifo_filled_cnt(io));
    let byte_slice = buffer.as_slice();
    // pprintln!("sd header 16bytes: {:x?}", &byte_slice[..2]);
}

/// for test driver
#[allow(unused)]
fn test_write_read<T: SDIo, S: SleepOps>(io: &mut T) {
    set_transaction_size(io, 512, 512);
    // write a block data
    let cmd24 = CmdReg::from(Cmd::WriteSingleBlock);
    let arg = CmdArg::new(0);
    let mut buffer: [u8; 512] = [0; 512];
    buffer.fill(u8::MAX);
    let _resp = send_cmd::<_, S>(
        io,
        Cmd::WriteSingleBlock,
        cmd24,
        arg,
        DataTransType::Write(&buffer),
    )
    .unwrap();
    // info!("resp csr: {:#?}",resp[0]); //csr reg
    // info!("Current FIFO count: {}", fifo_filled_cnt(io));
    // read a block data
    let cmd17 = CmdReg::from(Cmd::ReadSingleBlock);
    let arg = CmdArg::new(0);
    let mut buffer: [u8; 512] = [0; 512];
    let _resp = send_cmd::<_, S>(
        io,
        Cmd::ReadSingleBlock,
        cmd17,
        arg,
        DataTransType::Read(&mut buffer),
    )
    .unwrap();
    // info!("resp csr: {:#?}",resp[0]); //csr reg
    // info!("Current FIFO count: {}", fifo_filled_cnt(io));
    let byte_slice = buffer.as_slice();
    // debug!("Head 16 bytes: {:#x?}", &byte_slice[..2]);
}

// send acmd51 to read csr reg
fn check_bus_width<T: SDIo, S: SleepOps>(io: &mut T, rca: u32) -> usize {
    let cmd55 = CmdReg::from(Cmd::AppCmd);
    let cmd_arg = CmdArg::new(rca << 16);
    let _resp = send_cmd::<_, S>(io, Cmd::AppCmd, cmd55, cmd_arg, DataTransType::None).unwrap();
    // send acmd51
    // 1. set transact size
    set_transaction_size(io, 8, 8);
    // 2. send command
    let acmd51 = CmdReg::from(Cmd::SendScr);
    let mut buffer: [u8; 512] = [0; 512]; // 512B
    send_cmd::<_, S>(
        io,
        Cmd::SendScr,
        acmd51,
        CmdArg::new(0),
        DataTransType::Read(&mut buffer),
    );
    // info!("Current FIFO count: {}", fifo_filled_cnt(io)); //2
    let resp = u64::from_be(read_fifo(io, FIFO_DATA_REG));
    // pprintln!("Bus width supported: {:b}", (resp >> 48) & 0xF);
    // info!("Current FIFO count: {}", fifo_filled_cnt(io)); //0
    0
}

fn check_csd<T: SDIo, S: SleepOps>(io: &mut T, rca: u32) {
    let cmd = CmdReg::from(Cmd::SendCsd);
    let resp = send_cmd::<_, S>(
        io,
        Cmd::SendCsd,
        cmd,
        CmdArg::new(rca << 16),
        DataTransType::None,
    )
    .unwrap();
    let status = resp[0];
    // pprintln!("status: {:b}", status);
}

fn select_card<T: SDIo, S: SleepOps>(io: &mut T, rca: u32) {
    let cmd7 = CmdReg::from(Cmd::SelectCard);
    let cmd_arg = CmdArg::new(rca << 16);
    let resp = send_cmd::<_, S>(io, Cmd::SelectCard, cmd7, cmd_arg, DataTransType::None).unwrap();
    let r1 = resp[0];
    // info!("status: {:b}", r1);
}

fn check_rca<T: SDIo, S: SleepOps>(io: &mut T) -> u32 {
    let cmd3 = CmdReg::from(Cmd::SendRelativeAddr);
    let resp = send_cmd::<_, S>(
        io,
        Cmd::SendRelativeAddr,
        cmd3,
        CmdArg::new(0),
        DataTransType::None,
    )
    .unwrap();
    let rca = resp[0] >> 16;
    // info!("rca: {:#x}", rca);
    // info!("card status: {:b}", resp[0] & 0xffff);
    rca
}

fn check_cid<T: SDIo, S: SleepOps>(io: &mut T) {
    let cmd2 = CmdReg::from(Cmd::AllSendCid);
    let resp = send_cmd::<_, S>(
        io,
        Cmd::AllSendCid,
        cmd2,
        CmdArg::new(0),
        DataTransType::None,
    );
    if let Some(resp) = resp {
        // to 128 bit
        let resp = resp[0] as u128
            | (resp[1] as u128) << 32
            | (resp[2] as u128) << 64
            | (resp[3] as u128) << 96;
        let cid = Cid::new(resp);
        // #[cfg(feature = "alloc")]
        // pprintln!("cid: {}", cid.fmt());
        // #[cfg(not(feature = "alloc"))]
        // pprintln!("cid: {:?}", cid);
    }
}

fn check_version<T: SDIo, S: SleepOps>(io: &mut T) -> u8 {
    // check voltage
    let cmd8 = CmdReg::from(Cmd::SendIfCond);
    let cmd8_arg = CmdArg::new(0x1aa);
    let resp = send_cmd::<_, S>(io, Cmd::SendIfCond, cmd8, cmd8_arg, DataTransType::None).unwrap();
    if (resp[0] & 0xaa) == 0 {
        // error!("card {} unusable", 0);
        // pprintln!("card version: 1.0");
        return 1;
    }
    // pprintln!("card voltage: {:#x?}", resp[0]);
    // pprintln!("card version: 2.0");
    2
}

fn check_big_support<T: SDIo, S: SleepOps>(io: &mut T) -> bool {
    loop {
        // send cmd55
        let cmd55 = CmdReg::from(Cmd::AppCmd);
        send_cmd::<_, S>(io, Cmd::AppCmd, cmd55, CmdArg::new(0), DataTransType::None);
        let cmd41 = CmdReg::from(Cmd::SdSendOpCond);
        let cmd41_arg = CmdArg::new((1 << 30) | (1 << 24) | 0xFF8000);
        let resp =
            send_cmd::<_, S>(io, Cmd::SdSendOpCond, cmd41, cmd41_arg, DataTransType::None).unwrap();
        // info!("ocr: {:#x?}", resp[0]);
        let ocr = resp[0];
        if ocr.get_bit(31) {
            // pprintln!("card is ready");
            if ocr.get_bit(30) {
                // pprintln!("card is high capacity");
            } else {
                // pprintln!("card is standard capacity");
            }
            break;
        }
        S::sleep_ms(10);
    }
    true
}

fn init_sdcard<T: SDIo, S: SleepOps>(io: &mut T) {
    // read DETECT_REG
    let detect = read_reg(io, CDETECT_REG);
    // info!("detect: {:#?}", CDetectReg::new(detect));
    // read POWER_REG
    let power = read_reg(io, POWER_REG);
    // info!("power: {:#?}", PowerReg::new(power));
    // read CLOCK_ENABLE_REG
    let clock_enable = read_reg(io, CLOCK_ENABLE_REG);
    // info!("clock_enable: {:#?}", ClockEnableReg::from(clock_enable));
    // read CARD_TYPE_REG
    let card_type = read_reg(io, CTYPE_REG);
    // info!("card_type: {:#?}", CardTypeReg::from(card_type));
    // read Control Register
    let control = read_reg(io, CTRL_REG);
    // info!("control: {:#?}", ControlReg::from(control));
    // read  bus mode register
    let bus_mode = read_reg(io, BUS_MODE_REG);
    // info!("bus_mode(DMA): {:#?}", BusModeReg::from(bus_mode));
    // read DMA Descriptor List Base Address Register
    let dma_desc_base_lower = read_reg(io, DBADDRL_REG);
    let dma_desc_base_upper = read_reg(io, DBADDRU_REG);
    let dma_desc_base: usize = dma_desc_base_lower as usize | (dma_desc_base_upper as usize) << 32;
    // info!("dma_desc_base: {:#x?}", dma_desc_base);
    // read clock divider register
    let clock_divider = read_reg(io, CLK_DIVIDER_REG);
    // info!("clock_divider: {:#?}", ClockDividerReg::from(clock_divider));

    // reset card clock to 400Mhz
    reset_clock::<_, S>(io);
    // reset fifo
    reset_fifo(io);

    // set data width --> 1bit
    let ctype = CardTypeReg::from(0).with_card_width4_1(0);
    write_reg(io, CTYPE_REG, ctype.into());

    // reset dma
    reset_dma(io);

    let ctrl = ControlReg::from(read_reg(io, CTRL_REG));
    // info!("ctrl: {:#?}", ctrl);

    // go idle state
    let cmd0 = CmdReg::from(Cmd::GoIdleState);
    // cmd0.response_expect().set(u1!(0));
    send_cmd::<_, S>(
        io,
        Cmd::GoIdleState,
        cmd0,
        CmdArg::new(0),
        DataTransType::None,
    );
    // pprintln!("card is in idle state");

    check_version::<_, S>(io);

    check_big_support::<T, S>(io);

    check_cid::<_, S>(io);
    let rca = check_rca::<_, S>(io);
    // pprintln!("rca: {:#x?}", rca);
    check_csd::<_, S>(io, rca);

    // let raw_int_status = RawInterruptStatusReg::from(read_reg(io,RAW_INT_STATUS_REG));
    // pprintln!("RAW_INT_STATUS_REG: {:#?}", raw_int_status);

    S::sleep_ms(1);

    select_card::<_, S>(io, rca);

    let status = StatusReg::from(read_reg(io, STATUS_REG));
    // info!("Now FIFO Count is {}", status.fifo_count());

    // check bus width
    check_bus_width::<_, S>(io, rca);
    // try read a block data
    test_read::<_, S>(io);
    // test_write_read();

    // info!("CTRL_REG: {:#?}", ControlReg::from(read_reg(io, CTRL_REG)));
    let raw_int_status = RawInterruptStatusReg::from(read_reg(io, RAW_INT_STATUS_REG));
    // info!("RAW_INT_STATUS_REG: {:#?}", raw_int_status);
    // Clear interrupt by writing 1
    write_reg(io, RAW_INT_STATUS_REG, raw_int_status.into());

    // pprintln!("init sd success");
}

#[derive(Debug, Copy, Clone)]
pub enum Vf2SdDriverError {
    InitError,
    ReadError,
    WriteError,
    TimeoutError,
    UnknownError,
}

impl Display for Vf2SdDriverError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Vf2SdDriverError::InitError => write!(f, "init error"),
            Vf2SdDriverError::ReadError => write!(f, "read error"),
            Vf2SdDriverError::WriteError => write!(f, "write error"),
            Vf2SdDriverError::TimeoutError => write!(f, "timeout error"),
            Vf2SdDriverError::UnknownError => write!(f, "unknown error"),
        }
    }
}

pub type Result<T> = core::result::Result<T, Vf2SdDriverError>;

fn read_block<T: SDIo, S: SleepOps>(io: &mut T, block: usize, buf: &mut [u8]) -> Result<usize> {
    assert_eq!(buf.len(), 512);
    set_transaction_size(io, 512, 512);
    let cmd17 = CmdReg::from(Cmd::ReadSingleBlock);
    let arg = CmdArg::new(block as u32);
    let _resp = send_cmd::<_, S>(
        io,
        Cmd::ReadSingleBlock,
        cmd17,
        arg,
        DataTransType::Read(buf),
    )
    .unwrap();
    // info!("Current FIFO count: {}", fifo_filled_cnt(io));
    Ok(buf.len())
}

fn write_block<T: SDIo, S: SleepOps>(io: &mut T, block: usize, buf: &[u8]) -> Result<usize> {
    assert_eq!(buf.len(), 512);
    set_transaction_size(io, 512, 512);
    let cmd24 = CmdReg::from(Cmd::WriteSingleBlock);
    let arg = CmdArg::new(block as u32);
    let _resp = send_cmd::<_, S>(
        io,
        Cmd::WriteSingleBlock,
        cmd24,
        arg,
        DataTransType::Write(buf),
    )
    .unwrap();
    // info!("Current FIFO count: {}", fifo_filled_cnt(io));
    Ok(buf.len())
}

/// Vf2SdDriver
///
/// # Example
/// ```rust no run
/// fn sleep(ms:usize){}
/// use visionfive2_sd::Vf2SdDriver;
/// let driver = Vf2SdDriver::new(sleep);
/// driver.init();
/// let mut buf = [0u8;512];
/// driver.read_block(0,&mut buf);
/// driver.write_block(0,&buf);
/// ```
pub struct Vf2SdDriver<T, S> {
    io: T,
    _sleep: core::marker::PhantomData<S>,
}

impl<T: SDIo, S: SleepOps> Vf2SdDriver<T, S> {
    pub fn new(io: T) -> Self {
        Self {
            io,
            _sleep: core::marker::PhantomData,
        }
    }
    pub fn init(&mut self) {
        init_sdcard::<T, S>(&mut self.io);
    }
    pub fn read_block(&mut self, block: usize, buf: &mut [u8]) {
        read_block::<_, S>(&mut self.io, block, buf).unwrap();
    }
    pub fn write_block(&mut self, block: usize, buf: &[u8]) {
        write_block::<_, S>(&mut self.io, block, buf).unwrap();
    }
}
