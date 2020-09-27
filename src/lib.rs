#![no_std]

use bit_field::BitField;
use core::{fmt, marker::PhantomData};

/// PCIe supports 65536 segments, each with 256 buses, each with 32 slots, each with 8 possible functions. We cram this into a `u32`:
///
/// 32                              16               8         3      0
///  +-------------------------------+---------------+---------+------+
///  |            segment            |      bus      | device  | func |
///  +-------------------------------+---------------+---------+------+
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
///
/// Endpoints have a Type-0 header, so the remainder of the header is of the form:
/// ```ignore
///     32                           16                              0
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
pub struct PciHeader<A>(PciAddress, PhantomData<A>)
where
    A: ConfigRegionAccess;

impl<A> PciHeader<A>
where
    A: ConfigRegionAccess,
{
    pub fn new(address: PciAddress) -> PciHeader<A> {
        PciHeader(address, PhantomData)
    }

    pub fn id(&self, access: &A) -> (VendorId, DeviceId) {
        let id = unsafe { access.read(self.0, 0x00) };
        (id.get_bits(0..16) as u16, id.get_bits(16..32) as u16)
    }

    pub fn header_type(&self, access: &A) -> u8 {
        /*
         * Read bits 0..=6 of the Header Type. Bit 7 dictates whether the device has multiple functions and so
         * isn't read here.
         */
        unsafe { access.read(self.0, 0x0c) }.get_bits(16..23) as u8
    }

    pub fn has_multiple_functions(&self, access: &A) -> bool {
        /*
         * Reads bit 7 of the Header Type, which is 1 if the device has multiple functions.
         */
        unsafe { access.read(self.0, 0x0c) }.get_bit(23)
    }

    pub fn revision_and_class(&self, access: &A) -> (DeviceRevision, BaseClass, SubClass, Interface) {
        let field = unsafe { access.read(self.0, 0x08) };
        (
            field.get_bits(0..8) as DeviceRevision,
            field.get_bits(24..32) as BaseClass,
            field.get_bits(16..24) as SubClass,
            field.get_bits(8..16) as Interface,
        )
    }
}
