// ============================================================================
// dag.rs — DAG (Directed Acyclic Graph) and topological ordering
// ============================================================================
//
// This module models target dependencies as a DAG and performs topological
// ordering with Kahn's algorithm. Cyclic dependencies are reported as errors.
// ============================================================================

use std::collections::{HashMap, HashSet, VecDeque};

use crate::config::ResolvedProject;

// ---------------------------------------------------------------------------
// Topological order result: build levels
// ---------------------------------------------------------------------------

/// Each level contains target names that are independent and can be built
/// in parallel. Levels must be processed in order.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BuildOrder {
    /// Ordered levels: targets in each level can be built in parallel
    pub levels: Vec<Vec<String>>,
}

// ---------------------------------------------------------------------------
// DAG construction and topological ordering
// ---------------------------------------------------------------------------

/// Analyzes target dependencies and determines build order via topological sort.
///
/// Kahn's algorithm:
/// 1. Compute in-degree of each node
/// 2. Enqueue nodes with in-degree 0
/// 3. Dequeue, decrease in-degree of dependents
/// 4. Repeat until all nodes processed
/// 5. If any node remains, there is a cycle
pub fn build_order(project: &ResolvedProject) -> Result<BuildOrder, String> {
    let targets = &project.targets;

    // --- Step 1: Validate dependencies ---
    for (name, target) in targets {
        for dep in &target.deps {
            if !targets.contains_key(dep) {
                return Err(format!(
                    "Target '{}' has unknown dependency '{}'. \
                     Defined targets: {:?}",
                    name,
                    dep,
                    targets.keys().collect::<Vec<_>>()
                ));
            }
        }
    }

    // --- Step 2: Compute in-degrees ---
    // in_degree[x] = number of deps of x (not number of dependents of x)
    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();

    for name in targets.keys() {
        in_degree.insert(name.as_str(), 0);
        dependents.insert(name.as_str(), Vec::new());
    }

    // A depends on B → A can be built after B. A's in_degree = dep count;
    // B's dependents list gets A.
    for (name, target) in targets {
        let dep_count = target.deps.len();
        *in_degree.get_mut(name.as_str()).unwrap() = dep_count;

        for dep in &target.deps {
            dependents
                .get_mut(dep.as_str())
                .unwrap()
                .push(name.as_str());
        }
    }

    // --- Step 3: Kahn algorithm, level-by-level ---
    let mut levels: Vec<Vec<String>> = Vec::new();
    let mut queue: VecDeque<&str> = VecDeque::new();
    let mut processed_count = 0;

    // Enqueue nodes with in-degree 0
    for (name, &degree) in &in_degree {
        if degree == 0 {
            queue.push_back(name);
        }
    }

    while !queue.is_empty() {
        // Collect all independent targets for this level
        let mut current_level: Vec<String> = Vec::new();
        let level_size = queue.len();

        for _ in 0..level_size {
            let node = queue.pop_front().unwrap();
            current_level.push(node.to_string());
            processed_count += 1;

            // Decrease in-degree of targets that depend on this node
            if let Some(deps) = dependents.get(node) {
                for &dependent in deps {
                    let degree = in_degree.get_mut(dependent).unwrap();
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push_back(dependent);
                    }
                }
            }
        }

        // Sort level for reproducible output
        current_level.sort();
        levels.push(current_level);
    }

    // --- Step 4: Cycle check ---
    if processed_count != targets.len() {
        let processed: HashSet<String> = levels.iter().flatten().cloned().collect();
        let cyclic: Vec<&String> = targets
            .keys()
            .filter(|k| !processed.contains(k.as_str()))
            .collect();

        return Err(format!(
            "Cyclic dependency detected! The following targets form a cycle: {:?}",
            cyclic
        ));
    }

    Ok(BuildOrder { levels })
}

// ---------------------------------------------------------------------------
// Filter build order for specific targets (and their dependencies)
// ---------------------------------------------------------------------------

/// Returns a build order containing only the targets in `target_names` and
/// their dependencies. Empty list = all targets.
pub fn filter_order_for_targets(
    project: &ResolvedProject,
    order: &BuildOrder,
    target_names: &[String],
) -> Result<BuildOrder, String> {
    if target_names.is_empty() {
        return Ok(BuildOrder {
            levels: order.levels.clone(),
        });
    }

    let targets = &project.targets;
    for name in target_names {
        if !targets.contains_key(name) {
            return Err(format!(
                "Target '{}' not found. Defined targets: {:?}",
                name,
                targets.keys().collect::<Vec<_>>()
            ));
        }
    }

    // Requested targets + all transitive dependencies
    let mut closure: HashSet<String> = target_names.to_vec().into_iter().collect();
    let mut stack: Vec<String> = target_names.to_vec();
    while let Some(name) = stack.pop() {
        if let Some(t) = targets.get(&name) {
            for dep in &t.deps {
                if closure.insert(dep.clone()) {
                    stack.push(dep.clone());
                }
            }
        }
    }

    // Filter levels: only targets in closure, skip empty levels
    let levels: Vec<Vec<String>> = order
        .levels
        .iter()
        .map(|level| {
            level
                .iter()
                .filter(|n| closure.contains(*n))
                .cloned()
                .collect()
        })
        .filter(|level: &Vec<String>| !level.is_empty())
        .collect();

    Ok(BuildOrder { levels })
}

// ---------------------------------------------------------------------------
// Link order: correct -l order for executable/shared_lib
// ---------------------------------------------------------------------------
/// Returns the target's transitive dependencies in link order (dependent first,
/// then its deps) so the linker can resolve undefined references.
pub fn transitive_deps_in_link_order(
    project: &ResolvedProject,
    target_name: &str,
    order: &BuildOrder,
) -> Vec<String> {
    let targets = &project.targets;
    let Some(main_target) = targets.get(target_name) else { return vec![] };
    let mut closure: HashSet<String> = main_target.deps.iter().cloned().collect();
    let mut stack: Vec<String> = main_target.deps.clone();
    while let Some(name) = stack.pop() {
        if let Some(t) = targets.get(&name) {
            for dep in &t.deps {
                if closure.insert(dep.clone()) {
                    stack.push(dep.clone());
                }
            }
        }
    }
    // Reverse topological order: katmanları sondan başa, her katmandaki isimleri ekle
    let mut result = Vec::new();
    for level in order.levels.iter().rev() {
        for name in level {
            if closure.contains(name) {
                result.push(name.clone());
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Compiler, ResolvedTarget, TargetType};
    use std::path::PathBuf;

    fn make_target(name: &str, deps: Vec<&str>) -> ResolvedTarget {
        ResolvedTarget {
            name: name.to_string(),
            target_type: TargetType::Executable,
            sources: vec![],
            include_dirs: vec![],
            lib_dirs: vec![],
            libs: vec![],
            flags: vec![],
            deps: deps.into_iter().map(String::from).collect(),
            compiler: Compiler::Gpp,
            output_dir: PathBuf::from("build"),
        }
    }

    #[test]
    fn test_linear_deps() {
        // C → B → A (A bağımsız, B A'ya bağımlı, C B'ye bağımlı)
        let mut targets = HashMap::new();
        targets.insert("A".to_string(), make_target("A", vec![]));
        targets.insert("B".to_string(), make_target("B", vec!["A"]));
        targets.insert("C".to_string(), make_target("C", vec!["B"]));

        let project = ResolvedProject {
            name: "test".to_string(),
            version: "0.1.0".to_string(),
            cxx_standard: None,
            targets,
        };

        let order = build_order(&project).unwrap();
        assert_eq!(order.levels.len(), 3);
        assert_eq!(order.levels[0], vec!["A"]);
        assert_eq!(order.levels[1], vec!["B"]);
        assert_eq!(order.levels[2], vec!["C"]);
    }

    #[test]
    fn test_parallel_targets() {
        // A ve B bağımsız, C her ikisine de bağımlı
        let mut targets = HashMap::new();
        targets.insert("A".to_string(), make_target("A", vec![]));
        targets.insert("B".to_string(), make_target("B", vec![]));
        targets.insert("C".to_string(), make_target("C", vec!["A", "B"]));

        let project = ResolvedProject {
            name: "test".to_string(),
            version: "0.1.0".to_string(),
            cxx_standard: None,
            targets,
        };

        let order = build_order(&project).unwrap();
        assert_eq!(order.levels.len(), 2);
        assert_eq!(order.levels[0], vec!["A", "B"]); // Paralel
        assert_eq!(order.levels[1], vec!["C"]);
    }

    #[test]
    fn test_cyclic_dependency() {
        // A → B → A (döngüsel)
        let mut targets = HashMap::new();
        targets.insert("A".to_string(), make_target("A", vec!["B"]));
        targets.insert("B".to_string(), make_target("B", vec!["A"]));

        let project = ResolvedProject {
            name: "test".to_string(),
            version: "0.1.0".to_string(),
            cxx_standard: None,
            targets,
        };

        let result = build_order(&project);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Cyclic dependency"));
    }
}
