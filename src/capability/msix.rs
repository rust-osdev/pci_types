use super::PciCapabilityAddress;
use crate::ConfigRegionAccess;
use bit_field::BitField;

#[derive(Clone, Copy, Debug)]
pub struct MsixCapability {
    pub(super) address: PciCapabilityAddress,
    table_size: u16,
    /// Table BAR in bits 0..3 and offset into that BAR in bits 3..31
    table: u32,
    /// Pending Bit Array BAR in bits 0..3 and offset into that BAR in bits 3..31
    pba: u32,
}

impl MsixCapability {
    pub(crate) fn new(
        address: PciCapabilityAddress,
        control: u16,
        access: impl ConfigRegionAccess,
    ) -> MsixCapability {
        let table_size = control.get_bits(0..11) + 1;
        let table = unsafe { access.read(address.address, address.offset + 0x04) };
        let pba = unsafe { access.read(address.address, address.offset + 0x08) };
        MsixCapability { address, table_size, table, pba }
    }

    /// Enable MSI-X on the specified device feature.
    ///
    /// Unlike with MSI, the MSI message data and delivery address is not contained within the
    /// capability, but instead in system memory, and pointed to by the BAR specified by
    /// [`MsixCapability::table_bar`] and [`MsixCapability::table_offset`]. The caller is therefore
    /// responsible for configuring this separately, as this crate does not have access to
    /// arbitrary physical memory.
    pub fn set_enabled(&mut self, enabled: bool, access: impl ConfigRegionAccess) {
        let mut control = unsafe { access.read(self.address.address, self.address.offset) };
        control.set_bit(31, enabled);
        unsafe {
            access.write(self.address.address, self.address.offset, control);
        }
    }

    pub fn enabled(&self, access: impl ConfigRegionAccess) -> bool {
        let control = unsafe { access.read(self.address.address, self.address.offset) };
        control.get_bit(31)
    }

    /// Enable/disable masking of all interrupts for this PCI function.
    ///
    /// Individual interrupt sources can be masked using mask field of the corresponding entry in
    /// the MSI-X table.
    pub fn set_function_mask(&mut self, mask: bool, access: impl ConfigRegionAccess) {
        let mut control = unsafe { access.read(self.address.address, self.address.offset) };
        control.set_bit(30, mask);
        unsafe {
            access.write(self.address.address, self.address.offset, control);
        }
    }

    pub fn function_mask(&self, access: impl ConfigRegionAccess) -> bool {
        let control = unsafe { access.read(self.address.address, self.address.offset) };
        control.get_bit(30)
    }

    /// The index of the BAR that contains the MSI-X table.
    pub fn table_bar(&self) -> u8 {
        self.table.get_bits(0..3) as u8
    }

    /// The offset, in bytes, of the MSI-X table within its BAR.
    pub fn table_offset(&self) -> u32 {
        /*
         * To form the offset into the BAR, we mask off (set to zero) the lowest 3 bits, but
         * they're retained as part of the offset.
         */
        self.table & !0b111
    }

    pub fn table_size(&self) -> u16 {
        self.table_size
    }

    pub fn pba_bar(&self) -> u8 {
        self.pba.get_bits(0..3) as u8
    }

    pub fn pba_offset(&self) -> u32 {
        /*
         * To form the offset into the BAR, we mask off (set to zero) the lowest 3 bits, but
         * they're retained as part of the offset.
         */
        self.pba & !0b111
    }
}
