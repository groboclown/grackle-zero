# cpuid

Uses several different libraries to poll for different mechanisms that identify the computer.

The invocation allows for one or more of these arguments:

* s - get the system id.
* o - get the OS name.
* u - get the user name.
* m - get the MAC address.
* i - get the IP addresses.
* c - get the CPU ID.
* d - get the primary drive's serial number.
* h - get the computer hostname.
* q - get the number of CPU cores.

## OS Implementation Notes

For Windows, these values tend to come from reading shared values in the registry, or through kernel32 system calls.  The solution here requires adding in DLL shims to block access to some of these APIs.  At the moment, the library has no such mechanism in place.

## Tests Disabled

Until all of these restrictions (or the majority) have blocks in the sandbox, these tests do not run (the 01-executables.rs test comments out this test run).  To compensate for this lack of checking, the top-level README includes notes about limitations.
