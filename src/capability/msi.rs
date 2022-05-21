use crate::{capability::PciCapabilityAddress, ConfigRegionAccess};
use bit_field::BitField;
use core::convert::TryFrom;

/// Specifies, how much MSI interrupts one device can have.
/// Device will modify lower bits of interrupt vector to send multiple messages, so interrupt block
/// must be aligned accordingly.
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum MultipleMessageSupport {
    /// Device can send 1 interrupt. No interrupt vector modification is happening here
    Int1 = 0b000,
    /// Device can send 2 interrupts
    Int2 = 0b001,
    /// Device can send 4 interrupts
    Int4 = 0b010,
    /// Device can send 8 interrupts
    Int8 = 0b011,
    /// Device can send 16 interrupts
    Int16 = 0b100,
    /// Device can send 32 interrupts
    Int32 = 0b101,
}

impl TryFrom<u8> for MultipleMessageSupport {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0b000 => Ok(MultipleMessageSupport::Int1),
            0b001 => Ok(MultipleMessageSupport::Int2),
            0b010 => Ok(MultipleMessageSupport::Int4),
            0b011 => Ok(MultipleMessageSupport::Int8),
            0b100 => Ok(MultipleMessageSupport::Int16),
            0b101 => Ok(MultipleMessageSupport::Int32),
            _ => Err(()),
        }
    }
}

/// When device should trigger the interrupt
#[derive(Debug)]
pub enum TriggerMode {
    Edge = 0b00,
    LevelAssert = 0b11,
    LevelDeassert = 0b10,
}

#[derive(Debug, Clone)]
pub struct MsiCapability {
    address: PciCapabilityAddress,
    per_vector_masking: bool,
    is_64bit: bool,
    multiple_message_capable: MultipleMessageSupport,
}

impl MsiCapability {
    pub(crate) fn new(address: PciCapabilityAddress, control: u16) -> MsiCapability {
        MsiCapability {
            address,
            per_vector_masking: control.get_bit(8),
            is_64bit: control.get_bit(7),
            multiple_message_capable:
                MultipleMessageSupport::try_from(control.get_bits(1..4) as u8)
                    .unwrap_or(MultipleMessageSupport::Int1),
        }
    }

    /// Does device supports masking individual vectors?
    #[inline]
    pub fn has_per_vector_masking(&self) -> bool {
        self.per_vector_masking
    }

    /// Is device using 64-bit addressing?
    #[inline]
    pub fn is_64bit(&self) -> bool {
        self.is_64bit
    }

    /// How much interrupts this device has?
    #[inline]
    pub fn get_multiple_message_capable(&self) -> MultipleMessageSupport {
        self.multiple_message_capable
    }

    /// Is MSI capability enabled?
    pub fn is_enabled(&self, access: &impl ConfigRegionAccess) -> bool {
        let reg = unsafe { access.read(self.address.address, self.address.offset) };
        reg.get_bit(0)
    }

    /// Enable or disable MSI capability
    pub fn set_enabled(&self, enabled: bool, access: &impl ConfigRegionAccess) {
        let mut reg = unsafe { access.read(self.address.address, self.address.offset) };
        reg.set_bit(0, enabled);
        unsafe { access.write(self.address.address, self.address.offset, reg) };
    }

    /// Set how much interrupts the device will use. If requested count is bigger than supported count,
    /// the second will be used.
    pub fn set_multiple_message_enable(
        &self,
        data: MultipleMessageSupport,
        access: &impl ConfigRegionAccess,
    ) {
        let mut reg = unsafe { access.read(self.address.address, self.address.offset) };
        reg.set_bits(4..7, (data.min(self.multiple_message_capable)) as u32);
        unsafe { access.write(self.address.address, self.address.offset, reg) };
    }

    /// Return how much interrupts the device is using
    pub fn get_multiple_message_enable(
        &self,
        access: &impl ConfigRegionAccess,
    ) -> MultipleMessageSupport {
        let reg = unsafe { access.read(self.address.address, self.address.offset) };
        MultipleMessageSupport::try_from(reg.get_bits(4..7) as u8)
            .unwrap_or(MultipleMessageSupport::Int1)
    }

    /// Set where the interrupts will be sent to
    ///
    /// # Arguments
    /// * `address` - Target Local APIC address (if not changed, can be calculated with `0xFEE00000 | (processor << 12)`)
    /// * `vector` - Which interrupt vector should be triggered on LAPIC
    /// * `trigger_mode` - When interrupt should be triggered
    /// * `access` - PCI Configuration Space accessor
    pub fn set_message_info(
        &self,
        address: u32,
        vector: u8,
        trigger_mode: TriggerMode,
        access: &impl ConfigRegionAccess,
    ) {
        unsafe { access.write(self.address.address, self.address.offset + 0x4, address) }
        let data_offset = if self.is_64bit { 0xC } else { 0x8 };
        let mut data =
            unsafe { access.read(self.address.address, self.address.offset + data_offset) };
        data.set_bits(0..8, vector as u32);
        data.set_bits(14..16, trigger_mode as u32);
        unsafe {
            access.write(
                self.address.address,
                self.address.offset + data_offset,
                data,
            )
        }
    }

    /// Get interrupt mask
    ///
    /// # Note
    /// Only supported on when device supports 64-bit addressing and per-vector masking. Otherwise
    /// returns `0`
    pub fn get_message_mask(&self, access: &impl ConfigRegionAccess) -> u32 {
        if self.is_64bit && self.per_vector_masking {
            unsafe { access.read(self.address.address, self.address.offset + 0x10) }
        } else {
            0
        }
    }

    /// Set interrupt mask
    ///
    /// # Note
    /// Only supported on when device supports 64-bit addressing and per-vector masking. Otherwise
    /// will do nothing
    pub fn set_message_mask(&self, access: &impl ConfigRegionAccess, mask: u32) {
        if self.is_64bit && self.per_vector_masking {
            unsafe { access.write(self.address.address, self.address.offset + 0x10, mask) }
        }
    }

    /// Get pending interrupts
    ///
    /// # Note
    /// Only supported on when device supports 64-bit addressing. Otherwise will return `0`
    pub fn get_pending(&self, access: &impl ConfigRegionAccess) -> u32 {
        if self.is_64bit {
            unsafe { access.read(self.address.address, self.address.offset + 0x14) }
        } else {
            0
        }
    }
}
