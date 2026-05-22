# riscv-hypervisor

A RISC-V 64 hypervisor in Rust, built for QEMU `virt`.

## What works

![status](status.png)

It boots, sets up memory management, and launches a guest virtual machine. A tiny piece of guest code sits in its own address space running in virtualized mode :(
## What doesn't

The hypervisor cannot handle traps from the guest yet. No devices are emulated, no interrupts are forwarded, and there is only one CPU. The guest runs in a loop and the hypervisor just watches. Next steps are trap handling, device models, and basic VM management.
