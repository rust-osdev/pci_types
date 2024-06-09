use crate::{ConfigRegionAccess, PciAddress};
use bit_field::BitField;
use core::fmt::Formatter;

mod msi;
mod msix;

pub use msi::{MsiCapability, MultipleMessageSupport, TriggerMode};
pub use msix::MsixCapability;

#[derive(Clone, Copy)]
pub struct PciCapabilityAddress {
    pub address: PciAddress,
    pub offset: u16,
}

impl core::fmt::Debug for PciCapabilityAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}, offset: {:02x}", self.address, self.offset)
    }
}

/// PCI capabilities
#[derive(Clone, Copy, Debug)]
pub enum PciCapability {
    /// Power management capability, Cap ID = `0x01`
    PowerManagement(PciCapabilityAddress),
    /// Accelerated graphics port capability, Cap ID = `0x02`
    AcceleratedGraphicsPort(PciCapabilityAddress),
    /// Vital product data capability, Cap ID = `0x3`
    VitalProductData(PciCapabilityAddress),
    /// Slot identification capability, Cap ID = `0x04`
    SlotIdentification(PciCapabilityAddress),
    /// Message signalling interrupts capability, Cap ID = `0x05`
    Msi(MsiCapability),
    /// CompactPCI HotSwap capability, Cap ID = `0x06`
    CompactPCIHotswap(PciCapabilityAddress),
    /// PCI-X capability, Cap ID = `0x07`
    PciX(PciCapabilityAddress),
    /// HyperTransport capability, Cap ID = `0x08`
    HyperTransport(PciCapabilityAddress),
    /// Vendor-specific capability, Cap ID = `0x09`
    Vendor(PciCapabilityAddress),
    /// Debug port capability, Cap ID = `0x0A`
    DebugPort(PciCapabilityAddress),
    /// CompactPCI Central Resource Control capability, Cap ID = `0x0B`
    CompactPCICentralResourceControl(PciCapabilityAddress),
    /// PCI Standard Hot-Plug Controller capability, Cap ID = `0x0C`
    PciHotPlugControl(PciCapabilityAddress),
    /// Bridge subsystem vendor/device ID capability, Cap ID = `0x0D`
    BridgeSubsystemVendorId(PciCapabilityAddress),
    /// AGP Target PCI-PCI bridge capability, Cap ID = `0x0E`
    AGP3(PciCapabilityAddress),
    /// PCI Express capability, Cap ID = `0x10`
    PciExpress(PciCapabilityAddress),
    /// MSI-X capability, Cap ID = `0x11`
    MsiX(MsixCapability),
    /// Unknown capability
    Unknown { address: PciCapabilityAddress, id: u8 },
}

impl PciCapability {
    fn parse(
        id: u8,
        address: PciCapabilityAddress,
        extension: u16,
        access: impl ConfigRegionAccess,
    ) -> Option<PciCapability> {
        match id {
            0x00 => None, // null capability
            0x01 => Some(PciCapability::PowerManagement(address)),
            0x02 => Some(PciCapability::AcceleratedGraphicsPort(address)),
            0x03 => Some(PciCapability::VitalProductData(address)),
            0x04 => Some(PciCapability::SlotIdentification(address)),
            0x05 => Some(PciCapability::Msi(MsiCapability::new(address, extension))),
            0x06 => Some(PciCapability::CompactPCIHotswap(address)),
            0x07 => Some(PciCapability::PciX(address)),
            0x08 => Some(PciCapability::HyperTransport(address)),
            0x09 => Some(PciCapability::Vendor(address)),
            0x0A => Some(PciCapability::DebugPort(address)),
            0x0B => Some(PciCapability::CompactPCICentralResourceControl(address)),
            0x0C => Some(PciCapability::PciHotPlugControl(address)),
            0x0D => Some(PciCapability::BridgeSubsystemVendorId(address)),
            0x0E => Some(PciCapability::AGP3(address)),
            0x10 => Some(PciCapability::PciExpress(address)),
            0x11 => Some(PciCapability::MsiX(MsixCapability::new(address, extension, access))),
            _ => Some(PciCapability::Unknown { address, id }),
        }
    }

    pub fn address(&self) -> PciCapabilityAddress {
        match *self {
            PciCapability::PowerManagement(address) => address,
            PciCapability::AcceleratedGraphicsPort(address) => address,
            PciCapability::VitalProductData(address) => address,
            PciCapability::SlotIdentification(address) => address,
            PciCapability::Msi(msi_cap) => msi_cap.address,
            PciCapability::CompactPCIHotswap(address) => address,
            PciCapability::PciX(address) => address,
            PciCapability::HyperTransport(address) => address,
            PciCapability::Vendor(address) => address,
            PciCapability::DebugPort(address) => address,
            PciCapability::CompactPCICentralResourceControl(address) => address,
            PciCapability::PciHotPlugControl(address) => address,
            PciCapability::BridgeSubsystemVendorId(address) => address,
            PciCapability::AGP3(address) => address,
            PciCapability::PciExpress(address) => address,
            PciCapability::MsiX(msix_cap) => msix_cap.address,
            PciCapability::Unknown { address, id: _ } => address,
        }
    }
}

pub struct CapabilityIterator<T: ConfigRegionAccess> {
    address: PciAddress,
    offset: u16,
    access: T,
}

impl<T: ConfigRegionAccess> CapabilityIterator<T> {
    pub(crate) fn new(address: PciAddress, offset: u16, access: T) -> CapabilityIterator<T> {
        CapabilityIterator { address, offset, access }
    }
}

impl<T: ConfigRegionAccess> Iterator for CapabilityIterator<T> {
    type Item = PciCapability;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.offset == 0 {
                return None;
            }
            let data = unsafe { self.access.read(self.address, self.offset) };
            let next_ptr = data.get_bits(8..16);
            let id = data.get_bits(0..8);
            let extension = data.get_bits(16..32) as u16;
            let cap = PciCapability::parse(
                id as u8,
                PciCapabilityAddress { address: self.address, offset: self.offset },
                extension,
                &self.access,
            );
            self.offset = next_ptr as u16;
            if let Some(cap) = cap {
                return Some(cap);
            }
        }
    }
}
