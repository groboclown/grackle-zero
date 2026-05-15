// SPDX-License-Identifier: MIT

// The Windows version of this executable does not use the MS CRT libraries,
// which means it all-around does not look anything like the POSIX version.
// So there's just a clean break between the two implementations.

#if defined(_WIN32)
#include <windows.h>
void __stdcall entry(void) {
    /*
    HANDLE inh = GetStdHandle(STD_INPUT_HANDLE);
    HANDLE outh = GetStdHandle(STD_OUTPUT_HANDLE);
    unsigned char ch = 0;
    DWORD nr = 0, nw = 0;

    if (inh == INVALID_HANDLE_VALUE || outh == INVALID_HANDLE_VALUE) {
        ExitProcess(1);
    }

    // Read the marker from the parent to indicate the parent is ready.
    if (!ReadFile(inh, &ch, 1, &nr, NULL) || nr != 1) {
        ExitProcess(1);
    }

    // Tell the parent the application logic is starting.
    ch = '1';
    if (!WriteFile(outh, &ch, 1, &nw, NULL) || nw != 1) {
        ExitProcess(1);
    }

    // Action.  Here, nothing is done.

    // Tell the parent the application's logic completed successfully.
    ch = '2';
    if (!WriteFile(outh, &ch, 1, &nw, NULL) || nw != 1) {
        ExitProcess(1);
    }
    */

    ExitProcess(0);
}
#else
// POSIX

#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>


int main(int argc, char **argv)
{
    /*
    ssize_t nr = 0, nw = 0;
    unsigned char ch = 0;

    // Read the marker from the parent to indicate the parent is ready.
    do {
        nr = read(STDIN_FILENO, &ch, 1);
    } while (nr < 0 && errno == EINTR);
    if (nr != 1) {
        return 2;
    }

    // Tell the parent the application logic is starting.
    ch = '1';
    do {
        count = write(STDOUT_FILENO, &ch, 1);
    } while (count < 0 && errno == EINTR);
    if (nw != 1) {
        return 3;
    }

    // Action.  Here, nothing is done.

    // Tell the parent the application's logic completed successfully.
    ch = '1';
    do {
        count = write(STDOUT_FILENO, &ch, 1);
    } while (count < 0 && errno == EINTR);
    if (nw != 1) {
        return 4;
    }
    */

    // Quit successfully.
    return 0;
}
#endif
