# simple-cfg

Identical to [`simple-c`](../simple-c/), but with the Windows "Control Flow Guard" compile option enabled.

This allows for further testing of the Control Flow Guard security requirements.  Without this compile flag, any "always on" requirement for Control Flow Guard will cause the executable to not run.

Microsoft Compiler (MSCV): https://learn.microsoft.com/en-us/cpp/build/reference/guard-enable-control-flow-guard?view=msvc-170
clang: https://github.com/mstorsjo/llvm-mingw/issues/301

If you're not using MSVC (`cl`), this target now requires LLVM `clang` + `lld`.
GNU `gcc`/`ld` does not reliably generate runnable no-CRT binaries with the
strict CFG metadata required by these tests.

## Verify

Run:

```console
make verify
```

On Windows, this validates:

- the executable imports only `KERNEL32.dll`
- Control Flow Guard is marked as enabled in load-config metadata

The verifier supports either `llvm-readobj` (MinGW/Clang environments) or `dumpbin` (MSVC environments).

On non-Windows hosts, `make verify` is a no-op and prints a skip message.
