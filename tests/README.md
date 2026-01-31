# Test Applications

This directory contains applications used to test out the communication system.  They attempt various techniques to break out of the jail.

All applications here use the same general process:

1. Take 1 CLI argument (use of the argument depends on the purpose of the application).
2. Read 1 byte from stdin.  The test does not need to check for it, but it will be the ASCII character '0'
3. Send the ASCII character byte '1' to stdout and flush stdout.
4. Perform the process that should generate an access violation.  This uses the argument, generally.
5. Send the ASCII character byte '2' to stdout and flush stdout.

"stderr" is used for sending status messages, such as error reporting.

## Future To Do Tests

* Write to a file.
* Open a TCP connection to localhost.
* Start the current process as an executable, and have that second invocation perform a blocked action (like read or write to files).
* Perform rowhammer attack example.
* Perform meltdown or spectre attack example to find a secret value in the parent.
