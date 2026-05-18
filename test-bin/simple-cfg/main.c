// SPDX-License-Identifier: MIT

// The tests use this executable to ensure that the Windows protections
// around Control Flow Guard (CFG).  It does not apply, or should even work,
// for non-Windows environments.
// Just like the simple-c test binary, this executable does not use the
// MS CRT libraries.

#if defined(_WIN32)
#include <windows.h>

#if defined(__clang__) || defined(__GNUC__)

// This executable intentionally builds freestanding and links with -nostdlib.
// That means it does not get the normal C runtime startup objects that
// usually provide PE load-config metadata for security features.
// As a side effect, this code must explicitly construct the entrypoint
// structures and functions.
//
// For CFG, the Windows loader expects the image load configuration to
// advertise CFG instrumentation.  The compiler flags in Makefile
// enable CFG (--guard-cf or /guard:cf), but without the stdlib creating
// these structures for us, it lands on this code to explicitly construct the 
// "_load_config_used" section with the guard check fields set.
//
// The "_load_config_used" is special: the linker/loader use it
// as the source for IMAGE_LOAD_CONFIG. lld expects a specific contract when
// _load_config_used is present:
// certain fields must reference linker-defined CFG symbols so lld can wire
// final values/tables into the load-config directory.
//
// The "__guard*" symbols are produced by lld when --guard-cf is enabled.
// They are intentionally unresolved here and become valid at link time.
//
// Note: this block is only for Clang-style toolchains. MSVC toolchains
// handle CFG metadata through their own startup/link flow.  GNUC does
// not currently support the CFG structures.
//
// Another unfortunate item: this requires inline assembly instead of a
// C struct initializer, because lld expects specific COFF relocations for:
//   - __guard_check_icall_fptr
//   - __guard_dispatch_icall_fptr
//   - __guard_fids_table
//   - __guard_fids_count
//   - __guard_flags
// A normal C initializer does not reliably produce the exact relocation
// forms lld validates for this freestanding no-CRT path.
//
// This block emits IMAGE_LOAD_CONFIG_DIRECTORY64 with the exact field
// offsets from the MinGW/Clang winnt.h definition:
//   GuardCFCheckFunctionPointer    @ 112 -> __guard_check_icall_fptr
//   GuardCFDispatchFunctionPointer @ 120 -> __guard_dispatch_icall_fptr
//   GuardCFFunctionTable           @ 128 -> __guard_fids_table
//   GuardCFFunctionCount           @ 136 -> __guard_fids_count
//   GuardFlags                     @ 144 -> __guard_flags
//
// Everything else is zero for this minimal executable.
//
// References:
//   - Microsoft IMAGE_LOAD_CONFIG_DIRECTORY64 docs:
//     https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-image_load_config_directory64
//   - winnt.h structure used by this build (via <windows.h>):
//     IMAGE_LOAD_CONFIG_DIRECTORY64 / struct _IMAGE_LOAD_CONFIG_DIRECTORY64
//     (fields: GuardCFCheckFunctionPointer, GuardCFDispatchFunctionPointer,
//     GuardCFFunctionTable, GuardCFFunctionCount, GuardFlags)
//   - lld COFF CFG warning/validation tests, including loadcfg-full.s:
//     https://sources.debian.org/src/llvm-toolchain-19/1%3A19.1.7-3/lld/test/COFF/guard-warnings.s

void *__guard_check_icall_fptr = 0;
void *__guard_dispatch_icall_fptr = 0;

#if defined(__x86_64__)
__asm__(
    ".section .rdata,\"dr\"\n"
    ".globl _load_config_used\n"
    ".balign 8\n"
    "_load_config_used:\n"
    "  .long 0x100\n"
    "  .fill 108,1,0\n"
    "  .quad __guard_check_icall_fptr\n"
    "  .quad __guard_dispatch_icall_fptr\n"
    "  .quad __guard_fids_table\n"
    "  .quad __guard_fids_count\n"
    "  .long __guard_flags\n"
    "  .fill 108,1,0\n");
#endif /* __x86_64__ */
#endif /* __clang__ or __GNUC__ */

void __stdcall entry(void) {
    ExitProcess(0);
}
#endif /* _WIN32 */
