use nix::{ioctl_read, ioctl_readwrite};

// struct gpiochip_info
#[repr(C)]
pub struct gpio_chip_info {
    pub name: [u8; super::v2::GPIO_MAX_NAME_SIZE],
    pub label: [u8; super::v2::GPIO_MAX_NAME_SIZE],
    pub lines: u32,
}

ioctl_read!(gpio_get_chipinfo, 0xB4, 0x01, gpio_chip_info);

ioctl_readwrite!(gpio_get_lineinfo_unwatch, 0xB4, 0x06, u32);
