// SPDX-License-Identifier: MIT

//! Set up the jail from within the child process.
//! This uses the Linux-specific 'landlock' capability, which allows the
//! child process to declare its restrictions.
//! Due to the way it works, a restriction cannot be undone, therefore we
//! can add a full-on set of restrictions to get everything locked down as much
//! as possible.
//! It has an [official website](https://landlock.io/).

use std::path::PathBuf;

use landlock::{
    ABI, Access, AccessFs, AccessNet, Compatible, LandlockStatus, Ruleset, RulesetAttr,
    RulesetCreatedAttr, Scope, path_beneath_rules,
};

use crate::runtime::error::SandboxError;

/// A structure that allows for easy execution of the sandbox mode.
/// Intended to be constructed before entering the fork, in order to
/// eliminate memory consumption while forked.
pub struct LandlockJail {
    ruleset: landlock::RulesetCreated,
}

impl LandlockJail {
    pub fn new(allowed_read_paths: &Vec<PathBuf>) -> Result<Self, SandboxError> {
        Ok(LandlockJail {
            ruleset: new_sandbox(allowed_read_paths)
                .map_err(|e| SandboxError::JailSetup(e.to_string()))?,
        })
    }

    /// Perform the restriction within the jail.
    /// Because this *must* run within the forked process,
    /// it will exit on error.  And, because the expectation is that
    /// all I/O is already constrained due to FD wiggling, it reports no
    /// logging information.
    /// 
    /// Note: landlock works by allocating an FD that contains the ruleset.
    /// That means the child must wait to close FDs until after the restriction is applied.
    pub fn restrict(self) {
        match self.ruleset.restrict_self() {
            Err(_) => exit_err(),
            Ok(r) => match r.landlock {
                // Landlock disabled in the kernel configuration.
                // Re-enable by prepending "landlock," to the content of the CONFIG_LSM in kernel compile, or
                // at boot time by setting the same content to the "lsm" kernel parameter
                LandlockStatus::NotEnabled => exit_err(),
                // Landlock not built into the current kernel.
                // To support it, build the kernel with CONFIG_SECURITY_LANDLOCK=y and
                // prepend "landlock," to the content of CONFIG_LSM.
                LandlockStatus::NotImplemented => exit_err(),
                // kernel_abi == None: landlock ABI matches kernel supported ABI.
                // kernel_abi == Some(val): kernel supports ABI > landlock ABI (some features may not be in use).
                // effective_ab == ABI::V6: kernel's support matches compiled support.
                // effective_abi < ABI::V6: kernel doesn't support the expected landlock capabilities.
                // effective_abi > ABI::V6: kernel supports more features.
                LandlockStatus::Available {
                    effective_abi,
                    kernel_abi,
                } => (),
            },
        }
    }
}

fn exit_err() {
    std::process::exit(255);
}

/// Set the sandbox mode using low-level errors.
fn new_sandbox(
    allowed_read_paths: &Vec<PathBuf>,
) -> Result<landlock::RulesetCreated, landlock::RulesetError> {
    let mut paths = Vec::new();
    paths.extend(allowed_read_paths.iter());

    let abi_min = ABI::V1;
    let abi_latest = ABI::V6;
    Ruleset::default()
        // Hard requirements:
        //   - no read or write access to any file (this will be softened later).
        .set_compatibility(landlock::CompatLevel::HardRequirement)
        .handle_access(AccessFs::from_all(abi_min))?
        // Best effort:
        .set_compatibility(landlock::CompatLevel::BestEffort)
        //   - no unix sockets (ABI >= 6).
        .scope(Scope::AbstractUnixSocket)?
        //   - no signals (ABI >= 6).
        .scope(Scope::Signal)?
        //   - no additional file access (newer versions have more file restrictions)
        .handle_access(AccessFs::from_all(abi_min))?
        //   - no TCP binding or connecting to TCP (ABI >=4).
        .handle_access(AccessNet::from_all(abi_latest))?
        // Finish up the set of restrictions.
        .create()?
        // Prepare what is allowed - reading the allowed paths.
        .add_rules(path_beneath_rules(paths, AccessFs::from_read(abi_min)))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn test_landlock_jail() {
        let allowed_paths = vec![PathBuf::from("/tmp"), PathBuf::from("/var/log")];
        let jail = new_sandbox(&allowed_paths);
        assert!(jail.is_ok());
    }
}
