#![no_std]

pub mod device_type;

use bit_field::BitField;
use core::{fmt, marker::PhantomData};

/// PCIe supports 65536 segments, each with 256 buses, each with 32 slots, each with 8 possible functions. We cram this into a `u32`:
///
/// ```ignore
/// 32                              16               8         3      0
///  +-------------------------------+---------------+---------+------+
///  |            segment            |      bus      | device  | func |
///  +-------------------------------+---------------+---------+------+
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
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

pub type VendorId = u16;
pub type DeviceId = u16;
pub type DeviceRevision = u8;
pub type BaseClass = u8;
pub type SubClass = u8;
pub type Interface = u8;

// TODO: documentation
pub trait ConfigRegionAccess: Send {
    fn function_exists(&self, address: PciAddress) -> bool;
    unsafe fn read(&self, address: PciAddress, offset: u16) -> u32;
    unsafe fn write(&self, address: PciAddress, offset: u16, value: u32);
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

    pub fn id(&self, access: &impl ConfigRegionAccess) -> (VendorId, DeviceId) {
        let id = unsafe { access.read(self.0, 0x00) };
        (id.get_bits(0..16) as u16, id.get_bits(16..32) as u16)
    }

    pub fn header_type(&self, access: &impl ConfigRegionAccess) -> u8 {
        /*
         * Read bits 0..=6 of the Header Type. Bit 7 dictates whether the device has multiple functions and so
         * isn't returned here.
         */
        unsafe { access.read(self.0, 0x0c) }.get_bits(16..23) as u8
    }

    pub fn has_multiple_functions(&self, access: &impl ConfigRegionAccess) -> bool {
        /*
         * Reads bit 7 of the Header Type, which is 1 if the device has multiple functions.
         */
        unsafe { access.read(self.0, 0x0c) }.get_bit(23)
    }

    pub fn revision_and_class(
        &self,
        access: &impl ConfigRegionAccess,
    ) -> (DeviceRevision, BaseClass, SubClass, Interface) {
        let field = unsafe { access.read(self.0, 0x08) };
        (
            field.get_bits(0..8) as DeviceRevision,
            field.get_bits(24..32) as BaseClass,
            field.get_bits(16..24) as SubClass,
            field.get_bits(8..16) as Interface,
        )
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
///     |              |              |   line       |   line       |
///     +--------------+--------------+--------------+--------------+
/// ```
pub struct EndpointHeader(PciAddress);

impl EndpointHeader {
    pub fn from_header(header: PciHeader, access: &impl ConfigRegionAccess) -> Option<EndpointHeader> {
        match header.header_type(access) {
            0x00 => Some(EndpointHeader(header.0)),
            _ => None,
        }
    }

    pub fn header(&self) -> PciHeader {
        PciHeader(self.0)
    }

    /// Get the contents of a BAR in a given slot. Empty bars will return `None`.
    ///
    /// ### Note
    /// 64-bit memory BARs use two slots, so if one is decoded in e.g. slot #0, this method should not be called
    /// for slot #1
    pub fn bar(&self, slot: u8, access: &impl ConfigRegionAccess) -> Option<Bar> {
        let offset = 0x10 + (slot as u16) * 4;
        let bar = unsafe { access.read(self.0, offset) };

        /*
         * If bit 0 is `0`, the BAR is in memory. If it's `1`, it's in I/O.
         */
        if bar.get_bit(0) == false {
            let prefetchable = bar.get_bit(3);
            let address = bar.get_bits(4..32) << 4;

            // TODO: if the bar is 64-bits, do we need to do this on both BARs?
            let size = unsafe {
                access.write(self.0, offset, 0xffffffff);
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

            match bar.get_bits(1..3) {
                0b00 => Some(Bar::Memory32 { address, size, prefetchable }),
                0b10 => {
                    let address = {
                        let mut address = address as u64;
                        // TODO: do we need to mask off the lower bits on this?
                        address.set_bits(32..64, unsafe { access.read(self.0, offset + 4) } as u64);
                        address
                    };
                    Some(Bar::Memory64 { address, size: size as u64, prefetchable })
                }
                // TODO: should we bother to return an error here?
                _ => panic!("BAR Memory type is reserved!"),
            }
        } else {
            Some(Bar::Io { port: bar.get_bits(2..32) })
        }
    }
}

pub const MAX_BARS: usize = 6;

#[derive(Clone, Copy, Debug)]
pub enum Bar {
    Memory32 { address: u32, size: u32, prefetchable: bool },
    Memory64 { address: u64, size: u64, prefetchable: bool },
    Io { port: u32 },
}
