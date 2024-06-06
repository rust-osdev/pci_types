use bit_field::BitField;
use core::{
    convert::TryFrom,
    fmt::{self, Debug, Formatter},
};

/// Slowest time that a device will assert DEVSEL# for any bus command except Configuration Space
/// read and writes
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DevselTiming {
    Fast = 0x0,
    Medium = 0x1,
    Slow = 0x2,
}

#[derive(Debug)]
pub struct TryFromDevselTimingError {
    number: u8,
}

impl fmt::Display for TryFromDevselTimingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "No discriminant in `DevselTiming` matches the value `{}`", self.number)
    }
}

impl TryFrom<u8> for DevselTiming {
    type Error = TryFromDevselTimingError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x0 => Ok(DevselTiming::Fast),
            0x1 => Ok(DevselTiming::Medium),
            0x2 => Ok(DevselTiming::Slow),
            number => Err(TryFromDevselTimingError { number }),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct StatusRegister(u16);

impl StatusRegister {
    pub fn new(value: u16) -> Self {
        StatusRegister(value)
    }

    /// Will be `true` whenever the device detects a parity error, even if parity error handling is disabled.
    pub fn parity_error_detected(&self) -> bool {
        self.0.get_bit(15)
    }

    /// Will be `true` whenever the device asserts SERR#.
    pub fn signalled_system_error(&self) -> bool {
        self.0.get_bit(14)
    }

    /// Will return `true`, by a master device, whenever its transaction
    /// (except for Special Cycle transactions) is terminated with Master-Abort.
    pub fn received_master_abort(&self) -> bool {
        self.0.get_bit(13)
    }

    /// Will return `true`, by a master device, whenever its transaction is terminated with Target-Abort.
    pub fn received_target_abort(&self) -> bool {
        self.0.get_bit(12)
    }

    /// Will return `true` whenever a target device terminates a transaction with Target-Abort.
    pub fn signalled_target_abort(&self) -> bool {
        self.0.get_bit(11)
    }

    /// The slowest time that a device will assert DEVSEL# for any bus command except
    /// Configuration Space read and writes.
    ///
    /// For PCIe always set to `Fast`
    pub fn devsel_timing(&self) -> Result<DevselTiming, TryFromDevselTimingError> {
        let bits = self.0.get_bits(9..11);
        DevselTiming::try_from(bits as u8)
    }

    /// This returns `true` only when the following conditions are met:
    /// - The bus agent asserted PERR# on a read or observed an assertion of PERR# on a write
    /// - the agent setting the bit acted as the bus master for the operation in which the error occurred
    /// - bit 6 of the Command register (Parity Error Response bit) is set to 1.
    pub fn master_data_parity_error(&self) -> bool {
        self.0.get_bit(8)
    }

    /// If returns `true` the device can accept fast back-to-back transactions that are not from
    /// the same agent; otherwise, transactions can only be accepted from the same agent.
    ///
    /// For PCIe always set to `false`
    pub fn fast_back_to_back_capable(&self) -> bool {
        self.0.get_bit(7)
    }

    /// If returns `true` the device is capable of running at 66 MHz; otherwise, the device runs at 33 MHz.
    ///
    /// For PCIe always set to `false`
    pub fn capable_66mhz(&self) -> bool {
        self.0.get_bit(5)
    }

    /// If returns `true` the device implements the pointer for a New Capabilities Linked list;
    /// otherwise, the linked list is not available.
    ///
    /// For PCIe always set to `true`
    pub fn has_capability_list(&self) -> bool {
        self.0.get_bit(4)
    }

    /// Represents the state of the device's INTx# signal. If returns `true` and bit 10 of the
    /// Command register (Interrupt Disable bit) is set to 0 the signal will be asserted;
    /// otherwise, the signal will be ignored.
    pub fn interrupt_status(&self) -> bool {
        self.0.get_bit(3)
    }
}

impl Debug for StatusRegister {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("StatusRegister")
            .field("parity_error_detected", &self.parity_error_detected())
            .field("signalled_system_error", &self.signalled_system_error())
            .field("received_master_abort", &self.received_master_abort())
            .field("received_target_abort", &self.received_target_abort())
            .field("signalled_target_abort", &self.signalled_target_abort())
            .field("devsel_timing", &self.devsel_timing())
            .field("master_data_parity_error", &self.master_data_parity_error())
            .field("fast_back_to_back_capable", &self.fast_back_to_back_capable())
            .field("capable_66mhz", &self.capable_66mhz())
            .field("has_capability_list", &self.has_capability_list())
            .field("interrupt_status", &self.interrupt_status())
            .finish()
    }
}

bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct CommandRegister: u16 {
        const IO_ENABLE = 1 << 0;
        const MEMORY_ENABLE = 1 << 1;
        const BUS_MASTER_ENABLE = 1 << 2;
        const SPECIAL_CYCLE_ENABLE = 1 << 3;
        const MEMORY_WRITE_AND_INVALIDATE = 1 << 4;
        const VGA_PALETTE_SNOOP = 1 << 5;
        const PARITY_ERROR_RESPONSE = 1 << 6;
        const IDSEL_STEP_WAIT_CYCLE_CONTROL = 1 << 7;
        const SERR_ENABLE = 1 << 8;
        const FAST_BACK_TO_BACK_ENABLE = 1 << 9;
        const INTERRUPT_DISABLE = 1 << 10;
        const _ = !0;
    }
}
