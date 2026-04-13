ENTRY(_start)
OUTPUT_FORMAT(elf32-i386)

PHDRS {
	boot PT_LOAD;
	text PT_LOAD;
	data PT_LOAD;
	rodata PT_LOAD;
}

SECTIONS {
	. = 0M;

	.multiboot ALIGN(1K) : {
		KEEP(*(.multiboot))
	} : boot

	. = 1M;

	.text ALIGN(4K) : {
		*(.text)
		*(.text.*)
	} : text

	.rodata ALIGN(4K) : {
		*(.rodata)
		*(.rodata.*)
	} : rodata

	.data ALIGN(4K) : {
		*(.data)
		*(.data.*)
	} : data

	.bss ALIGN(4K) : {
		*(COMMON)
		*(.bss)
		*(.bss.*)
	} : data

	/* Guard page: 4KB unmapped zone below the kernel stack.
	   A stack overflow will hit this page and trigger a page fault
	   instead of silently corrupting the .bss section above. */
	. = ALIGN(4K);
	kernel_guard_page = .;
	. += 4K;

	/* Kernel stack: 16KB, grows downward from kernel_stack_top */
	.kernel_stack ALIGN(4K) : {
		kernel_stack_bottom = .;
		. += 16K;
		kernel_stack_top = .;
	} : data

	/* Guard page: 4KB unmapped zone below the user stack. */
	. = ALIGN(4K);
	user_guard_page = .;
	. += 4K;

	/* User stack: 8KB, grows downward from user_stack_top */
	.user_stack ALIGN(4K) : {
		user_stack_bottom = .;
		. += 8K;
		user_stack_top = .;
	} : data

	/* End of kernel image (page-aligned) — everything after this is free */
	. = ALIGN(4K);
	_kernel_end = .;
}
