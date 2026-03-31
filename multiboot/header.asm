		bits 32
		section .multiboot
		dd 0x1BADB002	; Magic number
		dd 0x0			; Flags
		dd - (0x1BADB002 + 0x0)	; Checksum

		section .text
		global _start

_start:
		; Set up kernel stack (16KB, defined in linker.ld)
		extern kernel_stack_top
		mov esp, kernel_stack_top

		; Kernel entry point
		extern kmain      ; Rust'taki ana fonksiyon
		call kmain        ; Rust kodunu çağır
		hlt                     ; CPU'yu durdur
		

; isr_keyboard.asm
global isr_keyboard

extern keyboard_interrupt_handler  ; Defined in Rust

section .text
isr_keyboard:
    pusha                       ; Save all general-purpose registers
    call keyboard_interrupt_handler
    popa                        ; Restore all general-purpose registers
    iret                        ; Return from interrupt

; ============================================================================
; CPU Exception ISRs
; ============================================================================

; Exceptions without error code (push dummy 0 for uniform handling)
global isr_division_error
global isr_debug
global isr_breakpoint
global isr_overflow
global isr_bound_range
global isr_invalid_opcode
global isr_device_not_available
global isr_coprocessor_segment
global isr_x87_floating_point
global isr_machine_check
global isr_simd_floating_point

; Exceptions with error code (CPU pushes error code)
global isr_double_fault
global isr_invalid_tss
global isr_segment_not_present
global isr_stack_segment_fault
global isr_general_protection_fault
global isr_page_fault
global isr_alignment_check
global isr_control_protection
global isr_hypervisor_injection
global isr_vmm_communication
global isr_security_exception

; Common macro-like pattern: pusha, call handler, popa, [pop error code], iret

isr_division_error:
    pusha
    extern exception_division_error
    call exception_division_error
    popa
    iret

isr_debug:
    pusha
    extern exception_debug
    call exception_debug
    popa
    iret

isr_breakpoint:
    pusha
    extern exception_breakpoint
    call exception_breakpoint
    popa
    iret

isr_overflow:
    pusha
    extern exception_overflow
    call exception_overflow
    popa
    iret

isr_bound_range:
    pusha
    extern exception_bound_range
    call exception_bound_range
    popa
    iret

isr_invalid_opcode:
    pusha
    extern exception_invalid_opcode
    call exception_invalid_opcode
    popa
    iret

isr_device_not_available:
    pusha
    extern exception_device_not_available
    call exception_device_not_available
    popa
    iret

isr_double_fault:
    pusha
    extern exception_double_fault
    call exception_double_fault
    popa
    add esp, 4                  ; Pop error code pushed by CPU
    iret

isr_coprocessor_segment:
    pusha
    extern exception_coprocessor_segment
    call exception_coprocessor_segment
    popa
    iret

isr_invalid_tss:
    pusha
    extern exception_invalid_tss
    call exception_invalid_tss
    popa
    add esp, 4                  ; Pop error code pushed by CPU
    iret

isr_segment_not_present:
    pusha
    extern exception_segment_not_present
    call exception_segment_not_present
    popa
    add esp, 4                  ; Pop error code pushed by CPU
    iret

isr_stack_segment_fault:
    pusha
    extern exception_stack_segment_fault
    call exception_stack_segment_fault
    popa
    add esp, 4                  ; Pop error code pushed by CPU
    iret

isr_general_protection_fault:
    pusha
    extern exception_general_protection_fault
    call exception_general_protection_fault
    popa
    add esp, 4                  ; Pop error code pushed by CPU
    iret

isr_page_fault:
    pusha                           ; Save all registers (32 bytes)
    mov eax, [esp + 32]            ; Grab error code (sits above pusha frame)
    push eax                        ; Push as cdecl argument
    extern exception_page_fault
    call exception_page_fault       ; Rust handler receives error_code: u32
    add esp, 4                      ; Clean up the argument we pushed
    popa                            ; Restore registers
    add esp, 4                      ; Pop error code pushed by CPU
    iret

isr_x87_floating_point:
    pusha
    extern exception_x87_floating_point
    call exception_x87_floating_point
    popa
    iret

isr_alignment_check:
    pusha
    extern exception_alignment_check
    call exception_alignment_check
    popa
    add esp, 4                  ; Pop error code pushed by CPU
    iret

isr_machine_check:
    pusha
    extern exception_machine_check
    call exception_machine_check
    popa
    iret

isr_simd_floating_point:
    pusha
    extern exception_simd_floating_point
    call exception_simd_floating_point
    popa
    iret

isr_control_protection:
    pusha
    extern exception_control_protection
    call exception_control_protection
    popa
    add esp, 4                  ; Pop error code pushed by CPU
    iret

isr_hypervisor_injection:
    pusha
    extern exception_hypervisor_injection
    call exception_hypervisor_injection
    popa
    iret

isr_vmm_communication:
    pusha
    extern exception_vmm_communication
    call exception_vmm_communication
    popa
    add esp, 4                  ; Pop error code pushed by CPU
    iret

isr_security_exception:
    pusha
    extern exception_security_exception
    call exception_security_exception
    popa
    add esp, 4                  ; Pop error code pushed by CPU
    iret

