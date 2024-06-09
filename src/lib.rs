#![no_std]

pub mod capability;
pub mod device_type;
mod register;

pub use register::{CommandRegister, DevselTiming, StatusRegister};

use crate::capability::CapabilityIterator;
use bit_field::BitField;
use core::fmt;

/// The address of a PCIe function.
///
/// PCIe supports 65536 segments, each with 256 buses, each with 32 slots, each with 8 possible functions. We pack this into a `u32`:
///
/// ```ignore
/// 32                              16               8         3      0
///  +-------------------------------+---------------+---------+------+
///  |            segment            |      bus      | device  | func |
///  +-------------------------------+---------------+---------+------+
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct PciAddress(u32);

impl PciAddress {
    pub fn new(segment: u16, bus: u8, device: u8, function: u8) -> PciAddress {
        let mut result = 0;
        result.set_bits(0..3, function as u32);
        result.set_bits(3..8, device as u32);
        result.set_bits(8..16, bus as u32);
        result.set_bits(16..32, segment as u32);
        PciAddress(result)
    }

    pub fn segment(&self) -> u16 {
        self.0.get_bits(16..32) as u16
    }

    pub fn bus(&self) -> u8 {
        self.0.get_bits(8..16) as u8
    }

    pub fn device(&self) -> u8 {
        self.0.get_bits(3..8) as u8
    }

    pub fn function(&self) -> u8 {
        self.0.get_bits(0..3) as u8
    }
}

impl fmt::Display for PciAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02x}-{:02x}:{:02x}.{}", self.segment(), self.bus(), self.device(), self.function())
    }
}

impl fmt::Debug for PciAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

pub type VendorId = u16;
pub type DeviceId = u16;
pub type DeviceRevision = u8;
pub type BaseClass = u8;
pub type SubClass = u8;
pub type Interface = u8;
pub type SubsystemId = u16;
pub type SubsystemVendorId = u16;
pub type InterruptLine = u8;
pub type InterruptPin = u8;

// TODO: documentation
pub trait ConfigRegionAccess {
    /// Performs a PCI read at `address` with `offset`.
    ///
    /// # Safety
    ///
    /// `address` and `offset` must be valid for PCI reads.
    unsafe fn read(&self, address: PciAddress, offset: u16) -> u32;

    /// Performs a PCI write at `address` with `offset`.
    ///
    /// # Safety
    ///
    /// `address` and `offset` must be valid for PCI writes.
    unsafe fn write(&self, address: PciAddress, offset: u16, value: u32);
}

impl<T: ConfigRegionAccess + ?Sized> ConfigRegionAccess for &T {
    #[inline]
    unsafe fn read(&self, address: PciAddress, offset: u16) -> u32 {
        (**self).read(address, offset)
    }

    #[inline]
    unsafe fn write(&self, address: PciAddress, offset: u16, value: u32) {
        (**self).write(address, offset, value)
    }
}

#[non_exhaustive]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HeaderType {
    Endpoint,
    PciPciBridge,
    CardBusBridge,
    Unknown(u8),
}

/// Every PCI configuration region starts with a header made up of two parts:
///    - a predefined region that identify the function (bytes `0x00..0x10`)
///    - a device-dependent region that depends on the Header Type field
///
/// The predefined region is of the form:
/// ```ignore
///     32                            16                              0
///      +-----------------------------+------------------------------+
///      |       Device ID             |       Vendor ID              | 0x00
///      |                             |                              |
///      +-----------------------------+------------------------------+
///      |         Status              |       Command                | 0x04
///      |                             |                              |
///      +-----------------------------+---------------+--------------+
///      |               Class Code                    |   Revision   | 0x08
///      |                                             |      ID      |
///      +--------------+--------------+---------------+--------------+
///      |     BIST     |    Header    |    Latency    |  Cacheline   | 0x0c
///      |              |     type     |     timer     |    size      |
///      +--------------+--------------+---------------+--------------+
/// ```
pub struct PciHeader(PciAddress);

impl PciHeader {
    pub fn new(address: PciAddress) -> PciHeader {
        PciHeader(address)
    }

    pub fn address(&self) -> PciAddress {
        self.0
    }

    pub fn id(&self, access: impl ConfigRegionAccess) -> (VendorId, DeviceId) {
        let id = unsafe { access.read(self.0, 0x00) };
        (id.get_bits(0..16) as VendorId, id.get_bits(16..32) as DeviceId)
    }

    pub fn header_type(&self, access: impl ConfigRegionAccess) -> HeaderType {
        /*
         * Read bits 0..=6 of the Header Type. Bit 7 dictates whether the device has multiple functions and so
         * isn't returned here.
         */
        match unsafe { access.read(self.0, 0x0c) }.get_bits(16..23) {
            0x00 => HeaderType::Endpoint,
            0x01 => HeaderType::PciPciBridge,
            0x02 => HeaderType::CardBusBridge,
            t => HeaderType::Unknown(t as u8),
        }
    }

    pub fn has_multiple_functions(&self, access: impl ConfigRegionAccess) -> bool {
        /*
         * Reads bit 7 of the Header Type, which is 1 if the device has multiple functions.
         */
        unsafe { access.read(self.0, 0x0c) }.get_bit(23)
    }

    pub fn revision_and_class(
        &self,
        access: impl ConfigRegionAccess,
    ) -> (DeviceRevision, BaseClass, SubClass, Interface) {
        let field = unsafe { access.read(self.0, 0x08) };
        (
            field.get_bits(0..8) as DeviceRevision,
            field.get_bits(24..32) as BaseClass,
            field.get_bits(16..24) as SubClass,
            field.get_bits(8..16) as Interface,
        )
    }

    pub fn status(&self, access: impl ConfigRegionAccess) -> StatusRegister {
        let data = unsafe { access.read(self.0, 0x4).get_bits(16..32) };
        StatusRegister::new(data as u16)
    }

    pub fn command(&self, access: impl ConfigRegionAccess) -> CommandRegister {
        let data = unsafe { access.read(self.0, 0x4).get_bits(0..16) };
        CommandRegister::from_bits_retain(data as u16)
    }

    pub fn update_command<F>(&mut self, access: impl ConfigRegionAccess, f: F)
    where
        F: FnOnce(CommandRegister) -> CommandRegister,
    {
        let mut data = unsafe { access.read(self.0, 0x4) };
        let new_command = f(CommandRegister::from_bits_retain(data.get_bits(0..16) as u16));
        data.set_bits(0..16, new_command.bits() as u32);
        unsafe {
            access.write(self.0, 0x4, data);
        }
    }
}

/// Endpoints have a Type-0 header, so the remainder of the header is of the form:
/// ```ignore
///     32                           16                              0
///     +-----------------------------------------------------------+ 0x00
///     |                                                           |
///     |                Predefined region of header                |
///     |                                                           |
///     |                                                           |
///     +-----------------------------------------------------------+
///     |                  Base Address Register 0                  | 0x10
///     |                                                           |
///     +-----------------------------------------------------------+
///     |                  Base Address Register 1                  | 0x14
///     |                                                           |
///     +-----------------------------------------------------------+
///     |                  Base Address Register 2                  | 0x18
///     |                                                           |
///     +-----------------------------------------------------------+
///     |                  Base Address Register 3                  | 0x1c
///     |                                                           |
///     +-----------------------------------------------------------+
///     |                  Base Address Register 4                  | 0x20
///     |                                                           |
///     +-----------------------------------------------------------+
///     |                  Base Address Register 5                  | 0x24
///     |                                                           |
///     +-----------------------------------------------------------+
///     |                  CardBus CIS Pointer                      | 0x28
///     |                                                           |
///     +----------------------------+------------------------------+
///     |       Subsystem ID         |    Subsystem vendor ID       | 0x2c
///     |                            |                              |
///     +----------------------------+------------------------------+
///     |               Expansion ROM Base Address                  | 0x30
///     |                                                           |
///     +--------------------------------------------+--------------+
///     |                 Reserved                   | Capabilities | 0x34
///     |                                            |   Pointer    |
///     +--------------------------------------------+--------------+
///     |                         Reserved                          | 0x38
///     |                                                           |
///     +--------------+--------------+--------------+--------------+
///     |   Max_Lat    |   Min_Gnt    |  Interrupt   |  Interrupt   | 0x3c
///     |              |              |   pin        |   line       |
///     +--------------+--------------+--------------+--------------+
/// ```
pub struct EndpointHeader(PciAddress);

impl EndpointHeader {
    pub fn from_header(header: PciHeader, access: impl ConfigRegionAccess) -> Option<EndpointHeader> {
        match header.header_type(access) {
            HeaderType::Endpoint => Some(EndpointHeader(header.0)),
            _ => None,
        }
    }

    pub fn header(&self) -> PciHeader {
        PciHeader(self.0)
    }

    pub fn status(&self, access: impl ConfigRegionAccess) -> StatusRegister {
        self.header().status(access)
    }

    pub fn command(&self, access: impl ConfigRegionAccess) -> CommandRegister {
        self.header().command(access)
    }

    pub fn update_command<F>(&mut self, access: impl ConfigRegionAccess, f: F)
    where
        F: FnOnce(CommandRegister) -> CommandRegister,
    {
        self.header().update_command(access, f);
    }

    pub fn capability_pointer(&self, access: impl ConfigRegionAccess) -> u16 {
        let status = self.status(&access);
        if status.has_capability_list() {
            unsafe { access.read(self.0, 0x34).get_bits(0..8) as u16 }
        } else {
            0
        }
    }

    pub fn capabilities<T: ConfigRegionAccess>(&self, access: T) -> CapabilityIterator<T> {
        let pointer = self.capability_pointer(&access);
        CapabilityIterator::new(self.0, pointer, access)
    }

    pub fn subsystem(&self, access: impl ConfigRegionAccess) -> (SubsystemId, SubsystemVendorId) {
        let data = unsafe { access.read(self.0, 0x2c) };
        (data.get_bits(16..32) as u16, data.get_bits(0..16) as u16)
    }

    /// Get the contents of a BAR in a given slot. Empty bars will return `None`.
    ///
    /// ### Note
    /// 64-bit memory BARs use two slots, so if one is decoded in e.g. slot #0, this method should not be called
    /// for slot #1
    pub fn bar(&self, slot: u8, access: impl ConfigRegionAccess) -> Option<Bar> {
        if slot >= 6 {
            return None;
        }

        let offset = 0x10 + (slot as u16) * 4;
        let bar = unsafe { access.read(self.0, offset) };

        /*
         * If bit 0 is `0`, the BAR is in memory. If it's `1`, it's in I/O.
         */
        if !bar.get_bit(0) {
            let prefetchable = bar.get_bit(3);
            let address = bar.get_bits(4..32) << 4;

            match bar.get_bits(1..3) {
                0b00 => {
                    let size = unsafe {
                        access.write(self.0, offset, 0xfffffff0);
                        let mut readback = access.read(self.0, offset);
                        access.write(self.0, offset, address);

                        /*
                         * If the entire readback value is zero, the BAR is not implemented, so we return `None`.
                         */
                        if readback == 0x0 {
                            return None;
                        }

                        readback.set_bits(0..4, 0);
                        1 << readback.trailing_zeros()
                    };
                    Some(Bar::Memory32 { address, size, prefetchable })
                }

                0b10 => {
                    /*
                     * If the BAR is 64 bit-wide and this slot is the last, there is no second slot to read.
                     */
                    if slot >= 5 {
                        return None;
                    }

                    let address_upper = unsafe { access.read(self.0, offset + 4) };

                    let size = unsafe {
                        access.write(self.0, offset, 0xfffffff0);
                        access.write(self.0, offset + 4, 0xffffffff);
                        let mut readback_low = access.read(self.0, offset);
                        let readback_high = access.read(self.0, offset + 4);
                        access.write(self.0, offset, address);
                        access.write(self.0, offset + 4, address_upper);

                        /*
                         * If the readback from the first slot is not 0, the size of the BAR is less than 4GiB.
                         */
                        readback_low.set_bits(0..4, 0);
                        if readback_low != 0 {
                            (1 << readback_low.trailing_zeros()) as u64
                        } else {
                            1u64 << ((readback_high.trailing_zeros() + 32) as u64)
                        }
                    };

                    let address = {
                        let mut address = address as u64;
                        // TODO: do we need to mask off the lower bits on this?
                        address.set_bits(32..64, address_upper as u64);
                        address
                    };

                    Some(Bar::Memory64 { address, size, prefetchable })
                }
                // TODO: should we bother to return an error here?
                _ => panic!("BAR Memory type is reserved!"),
            }
        } else {
            Some(Bar::Io { port: bar.get_bits(2..32) << 2 })
        }
    }

    /// Write to a BAR, setting the address for a device to use.
    ///
    /// # Safety
    ///
    /// The supplied value must be a valid BAR value (refer to the PCIe specification for
    /// requirements) and must be of the correct size (i.e. no larger than `u32::MAX` for 32-bit
    /// BARs). In the case of a 64-bit BAR, the supplied slot should be the first slot of the pair.
    pub unsafe fn write_bar(
        &mut self,
        slot: u8,
        access: impl ConfigRegionAccess,
        value: usize,
    ) -> Result<(), BarWriteError> {
        match self.bar(slot, &access) {
            Some(Bar::Memory64 { .. }) => {
                let offset = 0x10 + (slot as u16) * 4;
                unsafe {
                    access.write(self.0, offset, value.get_bits(0..32) as u32);
                    access.write(self.0, offset + 4, value.get_bits(32..64) as u32);
                }
                Ok(())
            }
            Some(Bar::Memory32 { .. }) | Some(Bar::Io { .. }) => {
                if value > u32::MAX as usize {
                    return Err(BarWriteError::InvalidValue);
                }

                let offset = 0x10 + (slot as u16) * 4;
                unsafe {
                    access.write(self.0, offset, value as u32);
                }
                Ok(())
            }
            None => Err(BarWriteError::NoSuchBar),
        }
    }

    pub fn interrupt(&self, access: impl ConfigRegionAccess) -> (InterruptPin, InterruptLine) {
        // According to the PCI Express Specification 4.0, Min_Gnt/Max_Lat registers
        // must be read-only and hardwired to 00h.
        let data = unsafe { access.read(self.0, 0x3c) };
        (data.get_bits(8..16) as u8, data.get_bits(0..8) as u8)
    }

    pub fn update_interrupt<F>(&mut self, access: impl ConfigRegionAccess, f: F)
    where
        F: FnOnce((InterruptPin, InterruptLine)) -> (InterruptPin, InterruptLine),
    {
        let mut data = unsafe { access.read(self.0, 0x3c) };
        let (new_pin, new_line) = f((data.get_bits(8..16) as u8, data.get_bits(0..8) as u8));
        data.set_bits(8..16, new_pin.into());
        data.set_bits(0..8, new_line.into());
        unsafe {
            access.write(self.0, 0x3c, data);
        }
    }
}

/// PCI-PCI Bridges have a Type-1 header, so the remainder of the header is of the form:
/// ```ignore
///     32                           16                              0
///     +-----------------------------------------------------------+ 0x00
///     |                                                           |
///     |                Predefined region of header                |
///     |                                                           |
///     |                                                           |
///     +-----------------------------------------------------------+
///     |                  Base Address Register 0                  | 0x10
///     |                                                           |
///     +-----------------------------------------------------------+
///     |                  Base Address Register 1                  | 0x14
///     |                                                           |
///     +--------------+--------------+--------------+--------------+
///     | Secondary    | Subordinate  |  Secondary   | Primary Bus  | 0x18
///     |Latency Timer | Bus Number   |  Bus Number  |   Number     |
///     +--------------+--------------+--------------+--------------+
///     |      Secondary Status       |  I/O Limit   |   I/O Base   | 0x1C
///     |                             |              |              |
///     +-----------------------------+--------------+--------------+
///     |        Memory Limit         |         Memory Base         | 0x20
///     |                             |                             |
///     +-----------------------------+-----------------------------+
///     |  Prefetchable Memory Limit  |  Prefetchable Memory Base   | 0x24
///     |                             |                             |
///     +-----------------------------+-----------------------------+
///     |             Prefetchable Base Upper 32 Bits               | 0x28
///     |                                                           |
///     +-----------------------------------------------------------+
///     |             Prefetchable Limit Upper 32 Bits              | 0x2C
///     |                                                           |
///     +-----------------------------+-----------------------------+
///     |   I/O Limit Upper 16 Bits   |   I/O Base Upper 16 Bits    | 0x30
///     |                             |                             |
///     +-----------------------------+--------------+--------------+
///     |              Reserved                      |  Capability  | 0x34
///     |                                            |   Pointer    |
///     +--------------------------------------------+--------------+
///     |                  Expansion ROM base address               | 0x38
///     |                                                           |
///     +-----------------------------+--------------+--------------+
///     |    Bridge Control           |  Interrupt   | Interrupt    | 0x3C
///     |                             |     PIN      |   Line       |
///     +-----------------------------+--------------+--------------+
/// ```
pub struct PciPciBridgeHeader(PciAddress);

impl PciPciBridgeHeader {
    pub fn from_header(header: PciHeader, access: impl ConfigRegionAccess) -> Option<PciPciBridgeHeader> {
        match header.header_type(access) {
            HeaderType::PciPciBridge => Some(PciPciBridgeHeader(header.0)),
            _ => None,
        }
    }

    pub fn header(&self) -> PciHeader {
        PciHeader(self.0)
    }

    pub fn status(&self, access: impl ConfigRegionAccess) -> StatusRegister {
        self.header().status(access)
    }

    pub fn command(&self, access: impl ConfigRegionAccess) -> CommandRegister {
        self.header().command(access)
    }

    pub fn update_command<F>(&mut self, access: impl ConfigRegionAccess, f: F)
    where
        F: FnOnce(CommandRegister) -> CommandRegister,
    {
        self.header().update_command(access, f);
    }

    pub fn primary_bus_number(&self, access: impl ConfigRegionAccess) -> u8 {
        let data = unsafe { access.read(self.0, 0x18).get_bits(0..8) };
        data as u8
    }

    pub fn secondary_bus_number(&self, access: impl ConfigRegionAccess) -> u8 {
        let data = unsafe { access.read(self.0, 0x18).get_bits(8..16) };
        data as u8
    }

    pub fn subordinate_bus_number(&self, access: impl ConfigRegionAccess) -> u8 {
        let data = unsafe { access.read(self.0, 0x18).get_bits(16..24) };
        data as u8
    }
}

pub const MAX_BARS: usize = 6;

#[derive(Clone, Copy, Debug)]
pub enum Bar {
    Memory32 { address: u32, size: u32, prefetchable: bool },
    Memory64 { address: u64, size: u64, prefetchable: bool },
    Io { port: u32 },
}

impl Bar {
    /// Return the IO port of this BAR or panic if not an IO BAR.
    pub fn unwrap_io(self) -> u32 {
        match self {
            Bar::Io { port } => port,
            Bar::Memory32 { .. } | Bar::Memory64 { .. } => panic!("expected IO BAR, found memory BAR"),
        }
    }

    /// Return the address and size of this BAR or panic if not a memory BAR.
    pub fn unwrap_mem(self) -> (usize, usize) {
        match self {
            Bar::Memory32 { address, size, prefetchable: _ } => (address as usize, size as usize),
            Bar::Memory64 { address, size, prefetchable: _ } => (
                address.try_into().expect("conversion from 64bit BAR to usize failed"),
                size.try_into().expect("conversion from 64bit BAR to usize failed"),
            ),
            Bar::Io { .. } => panic!("expected memory BAR, found IO BAR"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BarWriteError {
    NoSuchBar,
    InvalidValue,
}
