# riscv-hypervisor

A RISC-V 64 hypervisor in Rust, built for QEMU `virt`.

## What works

![status](status.png)

It boots, sets up memory management, loads a Linux kernel image with a generated device tree blob (DTB), and launches it in VS-mode (virtualized supervisor mode) with 2-stage address translation (hgatp).

## What doesn't

No devices are emulated and no interrupts are forwarded. The guest traps to the hypervisor but only panics -- no I/O emulation or device models exist yet. Only one CPU. Next steps are device models, interrupt injection, and SMP support.
