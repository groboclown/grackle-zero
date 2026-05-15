# simple-c

Performs just the required operations by the communication protocol.  Does no other action.  It uses C as the language, to eliminate as many library dependencies as possible.

This ensures that the process passes all sandbox checks, as it doesn't attempt any out-of-band actions.

## Windows Inspection

This program helps test out many of the restrictions options in the Windows implementation.  By separating itself from the MS CRT library that gives POSIX-like compatibility, it allows for running with the extreme restrictions set.

This assumes that you have the `mingw64` tool set installed.

To check which DLLs the executable contains, run:

```powershell
objdump -p target\debug\simple-c.exe | findstr "DLL Name"
```
