# exec-self

Attempts to run the current process as a child process.  As the process should not have access to other files, another path for the program to try is by running itself.  If it can run itself, there exists the possibility its new process has fewer restrictions than the launching program.
