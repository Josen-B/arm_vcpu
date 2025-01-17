extern crate alloc;
use crate::vcpu::Aarch64VCpu;
use aarch64_cpu::registers::{Readable, Writeable};
use aarch64_cpu::registers::{CNTFRQ_EL0, CNTPCT_EL0, CNTP_CTL_EL0, CNTP_TVAL_EL0};
use aarch64_sysreg::SystemRegType;
use alloc::sync::Arc;
use alloc::vec::Vec;

use axvcpu::{AxVCpu, AxVCpuHal, AxVcpuFunction};
use spin::RwLock;

use axhal::irq::MyVgic;

type RegVcpu<H> = Arc<AxVCpu<Aarch64VCpu<H>>>;

use core::arch::asm;
fn get_mpidr() -> u64 {
    let mpidr: u64;
    unsafe {
        asm!(
          "mrs {mpidr}, MPIDR_EL1", // 从 MPIDR_EL1 读取
          mpidr = out(reg) mpidr
        );
    }
    mpidr
}

/// Struct representing an entry in the emulator register list.
pub struct EmuRegEntry<H: AxVCpuHal> {
    /// The type of the emulator register.
    pub emu_type: EmuRegType,
    /// The address associated with the emulator register.
    pub addr: SystemRegType,
    /// The handler write function for the emulator register.
    pub handle_write: fn(SystemRegType, u64, RegVcpu<H>) -> AxVcpuFunction,
    /// The handler read function for the emulator register.
    pub handle_read: fn(SystemRegType, usize, RegVcpu<H>) -> AxVcpuFunction,
}

/// Enumeration representing the type of emulator registers.
pub enum EmuRegType {
    /// System register type for emulator registers.
    SysReg,
}

/// Struct representing the emulator registers.
pub struct Aarch64EmuRegs<H: AxVCpuHal> {
    /// The list of emulator registers.
    pub emu_regs: RwLock<Vec<EmuRegEntry<H>>>,
}

impl<H: AxVCpuHal> Aarch64EmuRegs<H> {
    const EMU_REGISTERS: [EmuRegEntry<H>; 3] = [
        EmuRegEntry {
            emu_type: EmuRegType::SysReg,
            addr: SystemRegType::CNTP_TVAL_EL0,
            handle_write: |addr, value, vcpu| {
                trace!(
                    "Write to emulator register: {:?}, value: {}, vcpu: {}, {}",
                    addr,
                    value,
                    vcpu.id(),
                    get_mpidr()
                );
                // CNTP_TVAL_EL0.set(value);
                let now = axhal::time::monotonic_time_nanos();
                trace!("Current time: {}, deadline: {}", now, value + now);
                // register_timer(
                //     value + now,
                //     VmmTimerEvent::new(|_| {
                //         trace!("Timer expired: {}", axhal::time::monotonic_time_nanos());
                //         let gich = MyVgic::get_gich();
                //         let hcr = gich.get_hcr();
                //         gich.set_hcr(hcr | 1 << 0);
                //         let mut lr = 0;
                //         lr |= 30 << 0;
                //         lr |= 1 << 19;
                //         lr |= 1 << 28;
                //         gich.set_lr(0, lr);
                //     }),
                // );
                // true
                AxVcpuFunction::SetTimer { deadline: value }
            },
            handle_read: |_, _, _| AxVcpuFunction::None,
        },
        EmuRegEntry {
            emu_type: EmuRegType::SysReg,
            addr: SystemRegType::CNTP_CTL_EL0,
            handle_write: |addr, value, _| {
                trace!("Write to emulator register: {:?}, value: {}", addr, value);
                // true
                AxVcpuFunction::None
            },
            handle_read: |_, _, _| AxVcpuFunction::None,
        },
        EmuRegEntry {
            emu_type: EmuRegType::SysReg,
            addr: SystemRegType::CNTPCT_EL0,
            handle_write: |_, _, _| AxVcpuFunction::None,
            handle_read: |addr, reg, vcpu| {
                let val = CNTPCT_EL0.get();
                trace!("Read from emulator register: {:?}, value: {}", addr, val);
                vcpu.set_gpr(reg, val as usize);
                // true
                AxVcpuFunction::None
            },
        },
    ];

    /// Handle a write to an emulator register.
    pub fn emu_register_handle_write(
        addr: SystemRegType,
        value: u64,
        vcpu: RegVcpu<H>,
    ) -> AxVcpuFunction {
        let emu_reg = Self::EMU_REGISTERS;

        for entry in emu_reg.iter() {
            if entry.addr == addr {
                return (entry.handle_write)(addr, value, vcpu);
            }
        }
        error!("Invalid emulated register write: {}", addr);
        // false
        AxVcpuFunction::None
    }

    /// Handle a read from an emulator register.
    pub fn emu_register_handle_read(
        addr: SystemRegType,
        reg: usize,
        vcpu: RegVcpu<H>,
    ) -> AxVcpuFunction {
        let emu_reg = Self::EMU_REGISTERS;

        for entry in emu_reg.iter() {
            if entry.addr == addr {
                return (entry.handle_read)(addr, reg, vcpu);
            }
        }
        error!("Invalid emulated register read: {}", addr);
        // false
        AxVcpuFunction::None
    }
}
