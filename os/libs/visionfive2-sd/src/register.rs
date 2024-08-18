use crate::cmd::Cmd;
use crate::utils::GetBit;
use bitfield_struct::bitfield;

pub const SDIO_BASE: usize = 0x16020000;
pub const CTRL_REG: usize = SDIO_BASE + 0x00;
pub const POWER_REG: usize = SDIO_BASE + 0x04;
pub const BLK_SIZE_REG: usize = SDIO_BASE + 0x1c;
pub const BYTE_CNT_REG: usize = SDIO_BASE + 0x20;
pub const CMD_REG: usize = SDIO_BASE + 0x2c;
pub const ARG_REG: usize = SDIO_BASE + 0x28;
pub const RESP0_REG: usize = SDIO_BASE + 0x30;
pub const RESP1_REG: usize = SDIO_BASE + 0x34;
pub const RESP2_REG: usize = SDIO_BASE + 0x38;
pub const RESP3_REG: usize = SDIO_BASE + 0x3c;
pub const STATUS_REG: usize = SDIO_BASE + 0x48;
pub const CDETECT_REG: usize = SDIO_BASE + 0x50;
pub const BUS_MODE_REG: usize = SDIO_BASE + 0x80;
pub const CTYPE_REG: usize = SDIO_BASE + 0x18;
pub const CLOCK_ENABLE_REG: usize = SDIO_BASE + 0x10;
pub const DBADDRL_REG: usize = SDIO_BASE + 0x88; // DMA DES Address Lower
pub const DBADDRU_REG: usize = SDIO_BASE + 0x8c; // DMA DES Address Upper
pub const CLK_DIVIDER_REG: usize = SDIO_BASE + 0x08;
pub const RAW_INT_STATUS_REG: usize = SDIO_BASE + 0x44;
pub const FIFO_DATA_REG: usize = SDIO_BASE + 0x600;

macro_rules! impl_into_u32 {
    ($name:ident) => {
        impl Into<u32> for $name {
            fn into(self) -> u32 {
                self.0
            }
        }
    };
}

macro_rules! impl_new {
    ($name:ident) => {
        impl $name {
            pub fn new(value: u32) -> Self {
                $name(value)
            }
        }
    };
}

#[derive(Debug)]
pub struct CmdArg(u32);
impl_new!(CmdArg);
impl_into_u32!(CmdArg);

/// Number of bytes to be transferred; should be integer multiple of Block Size for block transfers.
#[derive(Debug)]
pub struct ByteCountReg(u32);
impl_new!(ByteCountReg);
impl_into_u32!(ByteCountReg);
#[derive(Debug)]
pub struct BlkSizeReg(u32);
impl_new!(BlkSizeReg);
impl_into_u32!(BlkSizeReg);

#[derive(Debug)]
pub struct PowerReg(u32);
impl_new!(PowerReg);
impl_into_u32!(PowerReg);

#[derive(Debug)]
pub struct CDetectReg(u32);
impl_new!(CDetectReg);
impl_into_u32!(CDetectReg);

#[bitfield(u32,order = Msb)]
pub struct CmdReg {
    pub start_cmd: bool,
    reserved: bool,
    /// Use Hold Register
    ///
    /// 0 - CMD and DATA sent to card bypassing HOLD Register
    ///
    /// 1 - CMD and DATA sent to card through the HOLD Register For more information,
    /// refer to “Host Controller Output Path Timing” on page 320.
    pub use_hold_reg: bool,
    pub volt_switch: bool,
    pub boot_mode: bool,
    pub disable_boot: bool,
    pub expect_boot_ack: bool,
    pub enable_boot: bool,
    pub ccs_expected: bool,
    pub read_ceata_device: bool,
    /// 0 - Normal command sequence
    /// 1 - Do not send commands, just update clock register value into card clock domain
    pub update_clock_registers_only: bool,
    #[bits(5)]
    pub card_number: u16,
    pub send_initialization: bool,
    pub stop_abort_cmd: bool,
    ///0 - Send command at once, even if previous data transfer has not completed
    ///
    /// 1 - Wait for previous data transfer completion before sending command
    ///
    /// The wait_prvdata_complete = 0 option typically used to query status of card
    /// during data transfer or to stop current data transfer; card_number should be same as in previous command.
    pub wait_prvdata_complete: bool,
    ///
    /// 0 - No stop command sent at end of data transfer
    ///
    /// 1 - Send stop command at end of data transfer
    /// Don't care if no data expected from card.
    pub send_auto_stop: bool,
    ///
    /// 0 - Block data transfer command
    ///
    /// 1 - Stream data transfer command Don’t care if no data expected.
    pub transfer_mode: bool,
    /// 0 - Read from card
    ///
    /// 1 - Write to card
    ///
    /// Don’t care if no data expected from card.
    pub transfer_dir: bool,
    /// 	0 - No data transfer expected (read/write) 1 - Data transfer expected (read/write)
    pub data_expected: bool,
    /// 0 - Do not check response CRC
    ///
    /// 1 - Check response CRC
    ///
    /// Some of command responses do not return valid CRC bits.
    ///
    /// Software should disable CRC checks for those commands in order to disable CRC checking by controller.
    pub check_response_crc: bool,
    /// 0 - Short response expected from card 1 - Long response expected from card
    pub response_length: bool,
    /// 0 - No response expected from card 1 - Response expected from card
    pub response_expect: bool,
    #[bits(6)]
    /// Command index
    pub cmd_index: u16,
}

#[bitfield(u32,order = Msb)]
pub struct ClockEnableReg {
    /// Low-power control for up to 16 SD card clocks and one MMC card clock supported.
    ///
    /// 0 - Non-low-power mode
    ///
    /// 1 - Low-power mode; stop clock when card in IDLE (should be normally set to only
    /// MMC and SD memory cards; for SDIO cards, if interrupts must be detected, clock should not be stopped).
    pub cclk_low_power: u16,
    ///
    /// Clock-enable control for up to 16 SD card clocks and one MMC card clock supported.
    ///
    /// 0 - Clock disabled
    ///
    /// 1 - Clock enabled
    pub clk_enable: u16,
}

#[bitfield(u32,order = Msb)]
pub struct CardTypeReg {
    /// One bit per card indicates if card is 8-bit:
    /// 0 - Non 8-bit mode
    ///
    /// 1 - 8-bit mode
    ///
    /// Bit[31] corresponds to card[15]; bit[16] corresponds to card[0].
    pub card_width8: u16,
    /// One bit per card indicates if card is 1-bit or 4-bit:
    /// 0 - 1-bit mode
    ///
    /// 1 - 4-bit mode
    ///
    /// Bit[15] corresponds to card[15], bit[0] corresponds to card[0].
    ///
    /// Only NUM_CARDS*2 number of bits are implemented.
    pub card_width4_1: u16,
}

#[bitfield(u32,order = Msb)]
pub struct ClockDividerReg {
    pub clk_divider3: u8,
    pub clk_divider2: u8,
    pub clk_divider1: u8,
    /// Clock divider-0 value. Clock division is 2* n. For example, value of 0 means
    ///
    /// divide by 2*0 = 0 (no division, bypass), value of 1 means divide by 2*1 = 2,
    /// value of “ff” means divide by 2*255 = 510, and so on.
    pub clk_divider0: u8,
}

#[bitfield(u32,order = Msb)]
pub struct RawInterruptStatusReg {
    /// Interrupt from SDIO card; one bit for each card. Bit[31] corresponds to Card[15],
    /// and bit[16] is for Card[0]. Writes to these bits clear them. Value of 1 clears bit and 0 leaves bit intact.
    ///
    /// 0 - No SDIO interrupt from card
    ///
    /// 1 - SDIO interrupt from card
    pub sdiojnterrupt: u16,
    /// Writes to bits clear status bit. Value of 1 clears status bit, and value of 0 leaves bit intact.
    /// Bits are logged regardless of interrupt mask status.
    pub int_status: u16,
}

#[bitfield(u32,order = Msb)]
pub struct BusModeReg {
    #[bits(21)]
    reserved: u32,
    /// Programmable Burst Length. These bits indicate the maximum number of beats to be performed
    /// in one IDMAC transaction. The IDMAC will always attempt to burst as specified in PBL
    /// each time it starts a Burst transfer on the host bus.
    /// The permissible values are 1,4, 8, 16, 32, 64, 128 and 256.
    /// This value is the mirror of MSIZE of FIFOTH register. In order to change this value,
    /// write the required value to FIFOTH register. This is an encode value as follows.
    #[bits(3)]
    pub pbl: u8,
    /// IDMAC Enable. When set, the IDMAC is enabled.
    /// DE is read/write.
    pub de: bool,
    /// 	Descriptor Skip Length. Specifies the number of HWord/Word/Dword (depending on 16/32/64-bit bus)
    /// to skip between two unchained descriptors. This is applicable only for dual buffer structure.
    /// DSL is read/write.
    #[bits(5)]
    pub dsl: u8,
    pub fd: bool,
    /// Software Reset. When set, the DMA Controller resets all its internal registers.
    /// SWR is read/write. It is automatically cleared after 1 clock cycle.
    #[bits(1)]
    pub swr: bool,
}

#[bitfield(u32,order = Msb)]
pub struct StatusReg {
    /// DMA request signal state; either dw_dma_req or ge_dma_req, depending on DW-DMA or Generic-DMA selection.
    pub dma_req: bool,
    ///DMA acknowledge signal state; either dw_dma_ack or ge_dma_ack, depending on DW-DMA or Generic-DMA selection.
    pub dma_ack: bool,
    /// FIFO count - Number of filled locations in FIFO
    #[bits(13)]
    pub fifo_count: u16,
    /// Index of previous response, including any auto-stop sent by core.
    #[bits(6)]
    pub response_index: u8,
    /// Data transmit or receive state-machine is busy
    pub data_state_mc_busy: bool,
    /// Inverted version of raw selected card_data[0] 0 - card data not busy 1 - card data busy
    pub data_busy: bool,
    /// Raw selected card_data[3]; checks whether card is present 0 - card not present
    ///
    /// 1 - card present
    pub data_3_status: bool,
    #[bits(4)]
    pub command_fsm_states: u8,
    ///  	FIFO is full status
    pub fifo_full: bool,
    pub fifo_empty: bool,
    /// FIFO reached Transmit watermark level; not qualified with data
    ///
    /// transfer.
    pub fifo_tx_watermark: bool,
    ///
    /// FIFO reached Receive watermark level; not qualified with data
    ///
    /// transfer.
    pub fifo_rx_watermark: bool,
}
#[bitfield(u32,order = Msb)]
pub struct ControlReg {
    #[bits(6)]
    reserved: u8,
    /// Present only for the Internal DMAC configuration; else, it is reserved.
    /// 0– The host performs data transfers through the slave interface
    /// 1– Internal DMAC used for data transfer
    pub use_internal_dmac: bool,
    /// External open-drain pullup:
    ///
    /// 0- Disable
    /// 1 - Enable
    /// Inverted value of this bit is output to ccmd_od_pullup_en_n port.
    /// When bit is set, command output always driven in open-drive mode;
    /// that is, DWC_mobile_storage drives either 0 or high impedance, and does not drive hard 1.
    pub enable_od_pullup: bool,
    /// Card regulator-B voltage setting; output to card_volt_b port.
    ///
    /// Optional feature; ports can be used as general-purpose outputs.
    #[bits(4)]
    pub card_voltage_b: u8,
    /// Card regulator-A voltage setting; output to card_volt_a port.
    ///
    /// Optional feature; ports can be used as general-purpose outputs.
    #[bits(4)]
    pub card_voltage_a: u8,
    #[bits(4)]
    pub reserved1: u8,
    /// 0 - Interrupts not enabled in CE-ATA device (nIEN = 1 in ATA control register)
    /// 1 - Interrupts are enabled in CE-ATA device (nIEN = 0 in ATA control register)
    /// Software should appropriately write to this bit after power-on reset or any other reset to CE-ATA device.
    /// After reset, usually CE-ATA device interrupt is disabled (nIEN = 1).
    /// If the host enables CE-ATA device interrupt, then software should set this bit.
    pub ceata_device_interrupt: bool,
    /// 0 - Clear bit if DWC_mobile_storage does not reset the bit.
    /// 1 - Send internally generated STOP after sending CCSD to CE-ATA device.
    pub send_auto_stop_ccsd: bool,
    ///
    /// 0 - Clear bit if DWC_mobile_storage does not reset the bit.
    ///
    /// 1 - Send Command Completion Signal Disable (CCSD) to CE-ATA device
    pub send_ccsd: bool,
    /// 0 - No change
    ///
    /// 1 - After suspend command is issued during read-transfer, software polls card to
    /// find when suspend happened. Once suspend occurs, software sets bit to reset data state-machine,
    /// which is waiting for next block of data. Bit automatically clears once data state­machine resets to idle.
    ///
    /// Used in SDIO card suspend sequence.
    pub abort_read_data: bool,
    ///
    /// 0 - No change
    ///
    /// 1 - Send auto IRQ response
    ///
    /// Bit automatically clears once response is sent.
    ///
    /// To wait for MMC card interrupts, host issues CMD40, and DWC_mobile_storage waits for
    /// interrupt response from MMC card(s). In meantime, if host wants DWC_mobile_storage
    /// to exit waiting for interrupt state, it can set this bit, at which time DWC_mobile_storage
    /// command state-machine sends CMD40 response on bus and returns to idle state.
    pub send_irq_response: bool,
    ///
    /// 0 - Clear read wait
    ///
    /// 1 - Assert read wait For sending read-wait to SDIO cards.
    pub read_wait: bool,
    ///
    /// 0 - Disable DMA transfer mode
    ///
    /// 1 - Enable DMA transfer mode
    pub dma_enable: bool,
    ///
    /// Global interrupt enable/disable bit:
    ///
    /// 0 - Disable interrupts
    ///
    /// 1 - Enable interrupts
    ///
    /// The int port is 1 only when this bit is 1 and one or more unmasked interrupts are set.
    pub int_enable: bool,
    reserved2: bool,
    /// 0 - No change
    ///
    /// 1 - Reset internal DMA interface control logic
    ///
    /// To reset DMA interface, firmware should set bit to 1. This bit is auto-cleared after two AHB clocks.
    pub dma_reset: bool,
    /// 0 - No change
    ///
    /// 1 - Reset to data FIFO To reset FIFO pointers
    ///
    /// To reset FIFO, firmware should set bit to 1. This bit is auto-cleared after completion of reset operation.
    pub fifo_reset: bool,
    ///
    /// 0 - No change
    ///
    /// 1 - Reset DWC_mobile_storage controller
    pub controller_reset: bool,
}

#[bitfield(u16,order = Msb)]
pub struct RawInterrupt {
    /// End-bit error (read)/write no CRC (EBE)
    pub ebe: bool,
    /// Auto command done (ACD)
    pub acd: bool,
    /// Start-bit error (SBE) /Busy Clear Interrupt (BCI)
    pub sbe: bool,
    /// Hardware locked write error (HLE)
    pub hle: bool,
    /// FIFO underrun/overrun error (FRUN)
    pub frun: bool,
    /// Data starvation-by-host timeout (HTO) /Volt_switch_int
    pub hto: bool,
    /// Data read timeout (DRTO)/Boot Data Start (BDS)
    pub drto: bool,
    /// Response timeout (RTO)/Boot Ack Received (BAR)
    pub rto: bool,
    /// Data CRC error (DCRC)
    pub dcrc: bool,
    /// Response CRC error (RCRC)
    pub rcrc: bool,
    /// Receive FIFO data request (RxDR)
    pub rxdr: bool,
    /// Transmit FIFO data request (TXDR)
    pub txdr: bool,
    /// Data transfer over (DtO)
    pub dto: bool,
    /// Command done (CD)
    pub command_done: bool,
    /// Response error (RE)
    pub response_err: bool,
    /// Card detect (Cd)
    pub card_dectect: bool,
}

// mid:u8,
// oid:u16,
// pnm:u32,
// prv:u8,
// psn:u32,
// reserved:u4,
// mdt:u12,
// crc:u7,
// zero:u1,

#[derive(Debug)]
pub struct Cid(u128);

#[allow(dead_code)]
impl Cid {
    pub fn new(value: u128) -> Self {
        Cid(value)
    }

    #[cfg(feature = "alloc")]
    pub fn fmt(&self) -> alloc::string::String {
        use alloc::format;
        use alloc::string::ToString;
        let mid = self.0.get_bits(120, 127) as u8;
        let oid = self.0.get_bits(104, 119) as u16; // 2char
        let oid = core::str::from_utf8(&oid.to_be_bytes())
            .unwrap()
            .to_string();
        let pnm = self.0.get_bits(64, 103) as u64; // 5char
        let pnm = core::str::from_utf8(&pnm.to_be_bytes()[0..5])
            .unwrap()
            .to_string();
        let prv_big = self.0.get_bits(60, 63) as u8; //
        let prv_small = self.0.get_bits(56, 59) as u8; //
        let prv = format!("{}.{}", prv_big, prv_small);
        let psn = self.0.get_bits(24, 55) as u32; //
        let year = self.0.get_bits(12, 19) as u8; //
        let month = self.0.get_bits(8, 11) as u8; //
        let mdt = format!("{}-{}", year as usize + 2000, month);
        let res = format!(
            "mid:{} oid:{} pnm:{} prv:{} psn:{} mdt:{}",
            mid, oid, pnm, prv, psn, mdt
        );
        res
    }

    pub fn mid(&self) -> u8 {
        self.0.get_bits(120, 127) as u8
    }

    #[cfg(feature = "alloc")]
    pub fn oid(&self) -> alloc::string::String {
        use alloc::string::ToString;
        let oid = self.0.get_bits(104, 119) as u16; // 2char
        let oid = core::str::from_utf8(&oid.to_be_bytes())
            .unwrap()
            .to_string();
        oid
    }
    #[cfg(feature = "alloc")]
    pub fn pnm(&self) -> alloc::string::String {
        use alloc::string::ToString;
        let pnm = self.0.get_bits(64, 103) as u64; // 5char
        let pnm = core::str::from_utf8(&pnm.to_be_bytes()[0..5])
            .unwrap()
            .to_string();
        pnm
    }
    #[cfg(feature = "alloc")]
    pub fn prv(&self) -> alloc::string::String {
        let prv_big = self.0.get_bits(60, 63) as u8; //
        let prv_small = self.0.get_bits(56, 59) as u8; //
        let prv = alloc::format!("{}.{}", prv_big, prv_small);
        prv
    }
    pub fn psn(&self) -> u32 {
        self.0.get_bits(24, 55) as u32
    }
    #[cfg(feature = "alloc")]
    pub fn mdt(&self) -> alloc::string::String {
        let year = self.0.get_bits(12, 19) as u8; //
        let month = self.0.get_bits(8, 11) as u8; //
        let mdt = alloc::format!("{}-{}", year as usize + 2000, month);
        mdt
    }
}

impl RawInterrupt {
    pub fn have_error(&mut self) -> bool {
        self.rto() || self.dcrc() || self.response_err() || self.drto() || self.sbe() || self.ebe()
    }
}

impl CmdReg {
    pub fn default(card_number: usize, cmd_number: u8) -> Self {
        let cmd = CmdReg::new()
            .with_start_cmd(true)
            .with_use_hold_reg(true)
            .with_response_expect(true)
            .with_wait_prvdata_complete(true)
            .with_check_response_crc(true)
            .with_card_number(card_number as u16)
            .with_cmd_index(cmd_number as u16);
        cmd
    }
    pub fn with_no_data(card_number: usize, cmd_number: u8) -> Self {
        let cmd = CmdReg::default(card_number, cmd_number);
        cmd
    }

    pub fn with_data(card_number: usize, cmd_number: u8) -> Self {
        let cmd = CmdReg::default(card_number, cmd_number).with_data_expected(true);
        cmd
    }
}

impl From<Cmd> for CmdReg {
    fn from(value: Cmd) -> Self {
        match value {
            Cmd::GoIdleState => {
                let cmd0 = CmdReg::with_no_data(0, value.into()).with_send_initialization(true);
                cmd0
            }
            Cmd::SendIfCond | Cmd::AppCmd | Cmd::SendRelativeAddr | Cmd::SelectCard => {
                let cmd = CmdReg::with_no_data(0, value.into());
                cmd
            }
            Cmd::SdSendOpCond => {
                let cmd41 = CmdReg::with_no_data(0, value.into()).with_check_response_crc(false);
                cmd41
            }
            Cmd::SendCsd => {
                let cmd9 = CmdReg::with_no_data(0, value.into()).with_check_response_crc(false);
                cmd9
            }
            Cmd::AllSendCid => {
                let cmd2 = CmdReg::with_no_data(0, value.into())
                    .with_check_response_crc(false)
                    .with_response_length(true);
                cmd2
            }
            Cmd::SendScr | Cmd::ReadSingleBlock => {
                let cmd = CmdReg::with_data(0, value.into());
                cmd
            }
            Cmd::WriteSingleBlock => {
                let cmd = CmdReg::with_data(0, value.into()).with_transfer_dir(true);
                cmd
            }
            _ => {
                panic!("Not implemented")
            }
        }
    }
}
