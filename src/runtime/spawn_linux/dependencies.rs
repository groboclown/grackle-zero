// SPDX-License-Identifier: MIT

//! Discover the files used to run the program.
//!
//! This inspects the executable and its associated shared libraries.

use std::{collections::HashSet, path::PathBuf};

/// A binary dependency.  If the `realpath` is None, then it could not be found.
pub struct Dependency {
    pub path: PathBuf,
    pub realpath: Option<PathBuf>,
    pub required: bool,
}

impl Dependency {
    fn from_path(path: &PathBuf, required: bool) -> Self {
        let abs = match std::path::absolute(&path) {
            Ok(p) => p,
            Err(_) => {
                return Dependency {
                    path: path.to_path_buf(),
                    realpath: None,
                    required,
                };
            }
        };

        if path.exists() {
            Dependency {
                path: path.to_path_buf(),
                realpath: Some(abs),
                required,
            }
        } else {
            Dependency {
                path: path.to_path_buf(),
                realpath: None,
                required,
            }
        }
    }

    fn from_library(lib: &lddtree::Library, required_set: &HashSet<String>) -> Self {
        Dependency {
            path: lib.path.clone(),
            realpath: lib.realpath.clone(),
            required: required_set.contains(&lib.name),
        }
    }

    pub fn exists(&self) -> bool {
        self.realpath.is_some()
    }

    pub fn best_path(&self) -> &PathBuf {
        match &self.realpath {
            Some(r) => r,
            None => &self.path,
        }
    }

    pub fn invalid(&self) -> bool {
        self.required && self.realpath.is_none()
    }

    fn not_visited(&self, visited: &mut HashSet<PathBuf>) -> bool {
        let r = self.best_path();
        let ret = !visited.contains(r);
        if ret {
            visited.insert(r.clone());
        }
        ret
    }
}

/// Discovers all binary dependencies for the executable.
pub fn find_bin_dependencies(exec: &PathBuf) -> Vec<Dependency> {
    // Only perform the inspection if the executable exists.
    let exec_dep = Dependency::from_path(exec, true);
    if exec_dep.realpath.is_none() {
        return vec![exec_dep];
    }

    let analyzer = lddtree::DependencyAnalyzer::new(PathBuf::from("/"));
    let mut visited = HashSet::new();
    println!("Finding dependencies for: {:?}", &exec_dep.best_path());
    let mut ret = vec![exec_dep];

    // Populate a search path by scanning the dependency tree.
    // The dependency tree can include files that themselves have dependencies.
    // However, that's only useful for scanning the *needed* libraries.  Instead,
    // This pulls in all declared libraries, as those might be used optionally.
    let deps = match analyzer.analyze(exec) {
        Ok(d) => d,
        Err(_) => {
            return ret;
        }
    };
    let required = load_required_libs(&deps);
    for lib in deps.libraries.values() {
        println!("Library: {:?}", lib.name);
        let dep = Dependency::from_library(lib, &required);
        if dep.not_visited(&mut visited) {
            println!("Found dependency: {:?}", dep.best_path());
            ret.push(dep);
        }
    }
    ret
}

fn load_required_libs(tree: &lddtree::DependencyTree) -> HashSet<String> {
    let mut ret = HashSet::new();
    for name in &tree.needed {
        ret.insert(name.clone());
    }
    for lib in tree.libraries.values() {
        for name in &lib.needed {
            ret.insert(name.clone());
        }
    }
    ret
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_ls() {
        // This module requires a Linux, so linux specific test.
        let p_exec = which::which("ls").unwrap();
        assert_eq!(p_exec.exists(), true);

        // At a minimum, it should have 1 resolved dependency.
        let deps = find_bin_dependencies(&p_exec.into());
        // let mut unresolved_count = 0;
        let mut found_count = 0;
        for d in deps {
            if d.exists() {
                found_count += 1;
                assert_eq!(d.best_path().exists(), true, "resolved should exist");
            } else {
                // unresolved_count += 1;
                assert_eq!(d.best_path().exists(), false, "resolved should not exist");
            }
        }
        assert_eq!(found_count > 0, true, "Must have at least 1 dependency");
    }
}
