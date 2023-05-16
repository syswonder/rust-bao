use spin::Mutex;

use crate::{
    arch::aarch64::{
        armv8_a::vm::VGicDscr,
        defs::PAGE_SIZE,
        gic::vgic::{
            vgic_emul_generic_access, vgic_emul_razwi, VGicHandlerInfo, GICD_REG_ICACTIVER_OFF,
            GICD_REG_ICENABLER_OFF, GICD_REG_ICPENDR_OFF, GICD_REG_IPRIORITYR_OFF,
        },
    },
    baocore::{
        emul::{EmulAccess, EmulMem},
        types::Vaddr,
        vm::{myvm, VM},
    },
    println,
    util::align_up,
};

use super::{gicd::GicdHw, gicv3::GicrHw, vgic::{vgicd_emul_handler, VGicIntr}, GIC};

pub const fn gicd_reg_mask(addr: Vaddr) -> u64 {
    addr & 0xffff
}

const fn gicr_reg_mask(addr: Vaddr) -> u64 {
    addr & 0x1ffff
}

fn vgicr_get_id(acc: &EmulAccess) -> u64 {
    (acc.addr - myvm().arch.vgicr_addr) / align_up(core::mem::size_of::<GicrHw>(), PAGE_SIZE) as u64
}

pub fn vgic_init(vm: &mut VM, vgic_dscrp: &VGicDscr) {
    vm.arch.vgicr_addr = vgic_dscrp.gicr_addr;

    vm.arch.vgicd.int_num = unsafe { GIC.get().unwrap().max_irqs };
    vm.arch.vgicd.typer = ((vm.arch.vgicd.int_num as u32 / 32 - 1) & 0b11111) // ITLN
        | ((vm.cpu_num as u32 - 1) << 5)        // CPU_NUM
        | ((10 - 1) << 19); // TYPER_IDBITS

    for _ in 0..vm.arch.vgicd.int_num {
        vm.arch.vgicd.interrupts.push(VGicIntr::new());
        // vm.arch.vgicd.interrupts[i].owner = NULL;
        // vm.arch.vgicd.interrupts[i].lock = SPINLOCK_INITVAL;
        // vm.arch.vgicd.interrupts[i].id = i + GIC_CPU_PRIV;
        // vm.arch.vgicd.interrupts[i].state = INV;
        // vm.arch.vgicd.interrupts[i].prio = GIC_LOWEST_PRIO;
        // vm.arch.vgicd.interrupts[i].cfg = 0;
        // vm.arch.vgicd.interrupts[i].route = GICD_IROUTER_INV;
        // vm.arch.vgicd.interrupts[i].phys.route = GICD_IROUTER_INV;
        // vm.arch.vgicd.interrupts[i].hw = false;
        // vm.arch.vgicd.interrupts[i].in_lr = false;
        // vm.arch.vgicd.interrupts[i].enabled = false;
    }

    let vgicd_emul = EmulMem {
        va_base: vgic_dscrp.gicd_addr,
        size: align_up(core::mem::size_of::<GicdHw>(), PAGE_SIZE),
        handler: vgicd_emul_handler,
    };
    vm.emul_add_mem(vgicd_emul);

    let vgicr_emul = EmulMem {
        va_base: vgic_dscrp.gicr_addr,
        size: align_up(core::mem::size_of::<GicrHw>(), PAGE_SIZE) * vm.cpu_num,
        handler: vgicr_emul_handler,
    };
    vm.emul_add_mem(vgicr_emul);
}

fn vgicr_emul_handler(acc: &EmulAccess) -> bool {
    let gicr_reg = gicr_reg_mask(acc.addr);
    println!("gicr_reg = {:#x?}", gicr_reg);
    let handler_info = match gicr_reg {
        GICR_REG_WAKER_OFF | GICR_REG_IGROUPR0_OFF => VGicHandlerInfo {
            reg_access: vgic_emul_razwi,
            regroup_base: 0,
            field_width: 0,
        },
        GICR_REG_ICENABLER0_OFF => VGicHandlerInfo {
            reg_access: vgic_emul_generic_access,
            regroup_base: GICD_REG_ICENABLER_OFF,
            field_width: 1,
        },
        GICR_REG_ICPENDR0_OFF => VGicHandlerInfo {
            reg_access: vgic_emul_generic_access,
            regroup_base: GICD_REG_ICPENDR_OFF,
            field_width: 1,
        },
        GICR_REG_ICACTIVER0_OFF => VGicHandlerInfo {
            reg_access: vgic_emul_generic_access,
            regroup_base: GICD_REG_ICACTIVER_OFF,
            field_width: 1,
        },
        _ => {
            if gicr_reg >= GICR_REG_IPRIORITYR_OFF && gicr_reg < (GICR_REG_IPRIORITYR_OFF + 0x20) {
                VGicHandlerInfo {
                    reg_access: vgic_emul_generic_access,
                    regroup_base: GICD_REG_IPRIORITYR_OFF,
                    field_width: 8,
                }
            } else {
                todo!("vgicr_emul_handler");
            }
        }
    };

    // todo: check alignment?
    let vgcir_id = vgicr_get_id(acc);
    let vcpu = myvm().get_vcpu_mut(vgcir_id);

    let _gicr_mutex = vcpu.arch.vgic_priv.vgicr.lock.lock();
    (handler_info.reg_access)(acc, &handler_info, true, vgcir_id);
    true
}

pub struct VGicR {
    pub lock: Mutex<()>,
    pub typer: u64,
    pub ctlr: u32,
    pub iidr: u32,
}

pub const VGIC_ENABLE_MASK: u32 = 0x2;

// ------------ GICR REGS ------------------

const GICR_REG_WAKER_OFF: u64 = 0x14;
const GICR_REG_IGROUPR0_OFF: u64 = 0x10080;
const GICR_REG_ICENABLER0_OFF: u64 = 0x10180;
const GICR_REG_ICPENDR0_OFF: u64 = 0x10280;
const GICR_REG_ICACTIVER0_OFF: u64 = 0x10380;
const GICR_REG_IPRIORITYR_OFF: u64 = 0x10400;
