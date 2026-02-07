//! List of syscall names to used in the filters.
//! 
//! This allows for a larger than minimal set of privileges because the executable
//! will generally need access to load dynamic libraries and perform some
//! basic thread setup, even if it doesn't use threads.

pub(crate) const ALLOW_LIST: &[&str] = &[
    "read",
    "write",
    "readv",
    "writev",
    "close",
    "pread64",
    "pwrite64",
    "access",
    "faccessat",
    "faccessat2",
    "fcntl",
    "lseek",
    "exit",
    "exit_group",
    "brk",
    "mmap",
    "mprotect",
    "mremap",
    "munmap",
    "madvise",
    "rt_sigaction",
    "rt_sigprocmask",
    "rt_sigreturn",
    "sigaltstack",
    "arch_prctl",
    "set_tid_address",
    "set_robust_list",
    "futex",
    "rseq",
    "getpid",
    "gettid",
    "getrandom",
    "fstat",
    "fstatat",
    "newfstatat",
    "prlimit64",
    "poll",

    // Rely on FD inheritance and FD closures before exec to add restrictions that this would otherwise let pass.
    "ioctl",

    // Some code uses threads, or sets up threads even if not used.
    "set_tid_address",
    "set_robust_list",
    "futex",
    "rseq",
    "rt_sigreturn",

    // Allow the command execution to happen.
    "execve",

    // For lazy loaded libraries, some limited use of openat is allowed.
    // This should be a conditional, but I can't figure out the right semantics
    // to get it to run.  Instead, we rely on landlock to prevent bad opens.
    "open",
    "openat",
    "openat2",
        //.add_rule_conditional(
        //    ScmpAction::Allow,
        //    ScmpSyscall::from_name("openat")?,
        //    &[
        //        // Only allow the access mode, not the creation flags.
        //        scmp_cmp!($arg0 & ((
        //            nix::libc::O_ACCMODE | nix::libc::O_CREAT | nix::libc::O_TRUNC | nix::libc::O_TMPFILE | nix::libc::O_APPEND
        //        ) as u64) == nix::libc::O_RDONLY as u64),
        //    ])?

    // Should prevent timers where possible, to prevent rowhammer and spectre and meltdown attacks.
    // "timer_create",
    // "clock_gettime",
];
