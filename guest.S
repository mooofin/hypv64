/*
 * tiny guest kernel that just loops forever .
 * sits at 0x100000 and jumps to itself :3
 */
.section .text
.global guest_boot
guest_boot:
    j guest_boot
