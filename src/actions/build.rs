use std::{
    collections::BTreeSet,
    error,
    fmt::{Debug, Display},
    fs::File,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    str::from_utf8,
};

use log::{info, trace, warn};
use semver::Version;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    artifact::{Artifact, ContentId, ScriptConsumer, ScriptProcessingError},
    manifest::{
        module::{self, open_module, ModuleInfo},
        project::ProjectInfo,
    },
    util::from_empty_database,
};

pub const SQL_EXTENSION: &str = "sql";

#[derive(Clone)]
enum Task {
    Module { module: ModuleInfo },
    Script { path: PathBuf },
}
impl Task {
    pub fn path(&self) -> &Path {
        match self {
            Task::Module { module, .. } => module.path.as_path(),
            Task::Script { path, .. } => path.as_path(),
        }
    }
}

fn canonicalize_dep_path(
    dep: &Path,
    module_dir: &Path,
    source_dir: &Path,
) -> Result<PathBuf, BuildError> {
    let noncanonical_path = {
        if dep.is_absolute() {
            source_dir.join(dep.strip_prefix("/").expect("Failed to relativize path"))
        } else {
            module_dir.join(dep)
        }
    };
    let path = match noncanonical_path.canonicalize() {
        Ok(p) => p,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Err(BuildError::DependencyDoesNotExist {
                module: module_dir.to_path_buf(),
                dep: noncanonical_path,
            })
        }
        Err(e) => return Err(e.into()),
    };
    if path.starts_with(source_dir) {
        Ok(path)
    } else {
        Err(BuildError::DependencyOutsideRoot {
            module: module_dir.to_path_buf(),
            dep: path,
        })
    }
}

fn dep_module_path(dep: &Path) -> &Path {
    if dep.is_dir() {
        dep
    } else {
        dep.parent()
            .expect("Canonical paths to files should always have a parent")
    }
}

fn push_module(
    path: PathBuf,
    stack: &mut Vec<Task>,
    completed: &BTreeSet<PathBuf>,
    root: &Path,
) -> Result<(), BuildError> {
    trace!("Scheduling module dependency {}", path.to_str().unwrap());
    debug_assert!(
        !completed.contains(&path),
        "Completed tasks must never be scheduled, to ensure that tasks are never \
        processed twice"
    );
    debug_assert!(path == path.canonicalize().unwrap());
    debug_assert!(path.is_dir());

    if let Some(start) = stack
        .iter()
        .enumerate()
        .find(|(_, t)| t.path() == path)
        .map(|(idx, _)| idx)
    {
        let cycle_path = stack
            .iter()
            .skip(start)
            .map(|t| t.path().to_path_buf())
            .collect();
        Err(BuildError::DependencyCycle(DependencyCycle {
            cycle_path,
            root: root.to_path_buf(),
        }))
    } else {
        let module = open_module(path)?;
        stack.push(Task::Module { module });
        Ok(())
    }
}

fn get_script_deps<'a, 'b>(path: &'a Path, module: &'b ModuleInfo) -> Option<&'b Vec<PathBuf>> {
    let script_name = path.file_name();
    debug_assert!(script_name.is_some());
    for script in module.scripts.iter() {
        if script.script.file_name() == script_name {
            return Some(&script.dependencies);
        }
    }

    None
}

fn push_script(
    path: PathBuf,
    stack: &mut Vec<Task>,
    source_dir: &Path,
    completed: &BTreeSet<PathBuf>,
    root: &Path,
) -> Result<(), BuildError> {
    trace!("Scheduling script dependency {}", path.to_str().unwrap());
    debug_assert!(
        !completed.contains(&path),
        "Completed tasks must never be scheduled, to ensure that tasks are never \
        processed twice"
    );
    debug_assert!(path == path.canonicalize().unwrap());
    debug_assert!(path.is_file());

    if let Some(start) = stack
        .iter()
        .enumerate()
        .find(|(_, t)| t.path() == path)
        .map(|(idx, _)| idx)
    {
        let cycle_path = stack
            .iter()
            .skip(start)
            .map(|t| t.path().to_path_buf())
            .collect();
        Err(BuildError::DependencyCycle(DependencyCycle {
            cycle_path,
            root: root.to_path_buf(),
        }))
    } else {
        stack.push(Task::Script { path });
        Ok(())
    }
}

fn defer_module(
    path: PathBuf,
    defer_stack: &mut Vec<PathBuf>,
    completed_tasks: &BTreeSet<PathBuf>,
) {
    debug_assert!(path.is_dir());
    if !completed_tasks.contains(&path) {
        trace!("Deferring submodule {}", path.to_str().unwrap());
        defer_stack.push(path);
    }
}

fn process_module_task(
    module: ModuleInfo,
    depend_stack: &mut Vec<Task>,
    defer_stack: &mut Vec<PathBuf>,
    source_dir: &Path,
    completed_tasks: &BTreeSet<PathBuf>,
) -> Result<bool, BuildError> {
    if let Some(parent) = module.path.parent() {
        // If our parent directory is a directory module, we depend on it implicitly. It
        // may not have been processed if the project contains horizontal dependencies.
        if parent.starts_with(&source_dir) && !completed_tasks.contains(parent) {
            push_module(
                parent.to_path_buf(),
                depend_stack,
                &completed_tasks,
                source_dir,
            )?;
            return Ok(false);
        }
    }
    for dep in module.module.dependencies.iter() {
        // Push module-level dependencies
        let dep_path = canonicalize_dep_path(&dep, &module.path, &source_dir)?;
        let dep_module = dep_module_path(&dep_path);
        if completed_tasks.contains(dep_module) {
            continue;
        } else if dep_module == module.path {
            // It is redundant for a module to depend on it's own script. A module is
            // never complete until all of it's scripts are complete. Ignore.
            warn!(
                "Module {} depends on itself or one of its own scripts; this is ignored.",
                dep_module.to_str().unwrap()
            );
        } else if !dep_module.starts_with(&source_dir) {
            return Err(BuildError::DependencyOutsideRoot {
                module: module.path.clone(),
                dep: dep_path,
            });
        } else {
            push_module(
                dep_module.to_path_buf(),
                depend_stack,
                &completed_tasks,
                source_dir,
            )?;
            return Ok(false);
        }
    }
    for script in module.scripts.iter() {
        // Push script-level dependencies which are outside of the module
        for dep in script.dependencies.iter() {
            let dep_path = canonicalize_dep_path(dep, &module.path, &source_dir)?;
            let dep_module = dep_module_path(&dep_path);
            if !dep_module.starts_with(&source_dir) {
                return Err(BuildError::DependencyOutsideRoot {
                    module: module.path.clone(),
                    dep: dep_path,
                });
            } else if dep_module != module.path {
                push_module(
                    dep_module.to_path_buf(),
                    depend_stack,
                    &completed_tasks,
                    source_dir,
                )?;
                return Ok(false);
            }
        }
    }

    for child_res in module.path.read_dir()? {
        // Push children which are .sql scripts, defer children which are submodules
        let child = child_res?.path();
        if completed_tasks.contains(&child) {
            continue;
        }

        let md = child.metadata()?;
        if md.is_dir() {
            defer_module(child, defer_stack, &completed_tasks);
        } else if md.is_file() && child.extension().map(|s| s.to_str()) == Some(Some(SQL_EXTENSION))
        {
            push_script(
                child,
                depend_stack,
                &source_dir,
                &completed_tasks,
                source_dir,
            )?;
            return Ok(false);
        }
    }

    Ok(true)
}

fn process_script_task(
    path: PathBuf,
    depend_stack: &mut Vec<Task>,
    source_dir: &Path,
    completed_tasks: &BTreeSet<PathBuf>,
) -> Result<bool, BuildError> {
    let module_path = path.parent().unwrap();
    let module = open_module(module_path.to_path_buf())?;
    if let Some(deps) = get_script_deps(&path, &module) {
        for dep in deps.iter() {
            let dep_path = canonicalize_dep_path(&dep, &module_path, &source_dir)?;
            if !completed_tasks.contains(&dep_path) {
                debug_assert!(
                    dep_module_path(&dep_path) == module_path,
                    "All dependencies outside of the current module were resolved previously"
                );
                if dep.extension().map(|s| s.to_str()) != Some(Some(SQL_EXTENSION)) {
                    return Err(BuildError::DependencyIllegal {
                        module: module.path.clone(),
                        dep: dep.to_path_buf(),
                    });
                }
                push_script(
                    dep_path,
                    depend_stack,
                    &source_dir,
                    &completed_tasks,
                    source_dir,
                )?;
                return Ok(false);
            }
        }
    }

    Ok(true)
}

/// Topographically sort the scripts in our project using Kahn's algorithm
pub fn build_project(info: &ProjectInfo) -> Result<BuildArtifact, BuildError> {
    info!(
        "Building {} version {}",
        info.project.title, info.project.version
    );

    // This is our output. We will be inserting scripts into this list in
    // topographic order.
    let mut scripts = Vec::with_capacity(32);

    // The stack of dependencies we are currently processing. Each task depends
    // on the tasks earlier in the stack. This is critical for detecting cycles;
    // when we attempt to push a task which is already on the stack, we know that
    // we have a cycle.
    let mut depend_stack: Vec<Task> = Vec::with_capacity(8);

    // Submodules we have encountered but are not dependencies of the current task.
    // These must be saved so that the can be processed later, but cannot be pushed
    // onto depend_stack, because they are not dependencies.
    let mut defer_stack = Vec::with_capacity(8);

    let mut completed_tasks = BTreeSet::<PathBuf>::new();
    let source_dir = info.source_dir();
    if !source_dir.exists() {
        warn!("No source directory found");
        info!("Build complete");
        return Ok(BuildArtifact::new(scripts, info));
    };

    push_module(
        source_dir.clone(),
        &mut depend_stack,
        &completed_tasks,
        &source_dir,
    )?;
    while !depend_stack.is_empty() || !defer_stack.is_empty() {
        if depend_stack.is_empty() {
            // It is only ever safe to schedule a deffered task when the stack
            // is empty, because a deffered task has no known dependencies.
            while let Some(task) = defer_stack.pop() {
                if !completed_tasks.contains(&task) {
                    push_module(task, &mut depend_stack, &completed_tasks, &source_dir)?;
                    break;
                }
            }
        }
        let task = match depend_stack.last().cloned() {
            Some(m) => m,
            _ => break,
        };
        match task {
            Task::Module { module } => {
                if !process_module_task(
                    module,
                    &mut depend_stack,
                    &mut defer_stack,
                    &source_dir,
                    &completed_tasks,
                )? {
                    continue;
                }
            }
            Task::Script { path } => {
                if !process_script_task(path, &mut depend_stack, &source_dir, &completed_tasks)? {
                    continue;
                }
            }
        }

        // If we've gotten this far with continuing the loop, then our task is complete.
        let path = match depend_stack.pop().unwrap() {
            Task::Module { module, .. } => module.path,
            Task::Script { path, .. } => {
                scripts.push(path.clone());
                path
            }
        };
        let first_time = completed_tasks.insert(path);
        debug_assert!(first_time, "A task is never processed twice");
    }

    info!("Build complete");
    Ok(BuildArtifact::new(scripts, info))
}

#[derive(Error, Debug)]
pub enum BuildError {
    #[error("Dependency {dep} of module {module} is outside the source directory")]
    DependencyOutsideRoot { module: PathBuf, dep: PathBuf },
    #[error("Dependency {dep} of module {module} does not exist")]
    DependencyDoesNotExist { module: PathBuf, dep: PathBuf },
    #[error("Dependency {dep} of module {module} is neither a module nor a SQL script")]
    DependencyIllegal { module: PathBuf, dep: PathBuf },
    #[error("{0}:\n{0:?}")]
    DependencyCycle(#[from] DependencyCycle),
    #[error("Script {0} does not exists")]
    ScriptDoesNotExist(PathBuf),
    #[error("I/O Error: {0}")]
    Io(#[from] io::Error),
    #[error("Could not read module manifest: {0}")]
    ModuleManifest(#[from] module::OpenError),
}

pub struct DependencyCycle {
    pub root: PathBuf,
    pub cycle_path: Vec<PathBuf>,
}
impl Display for DependencyCycle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "A cycle exists between {0} and {1}",
            self.cycle_path
                .first()
                .expect("Path is nonempty")
                .strip_prefix(&self.root)
                .unwrap_or(self.cycle_path.first().unwrap())
                .to_str()
                .expect("Couldn't process path string"),
            self.cycle_path
                .last()
                .expect("Path is nonempty")
                .strip_prefix(&self.root)
                .unwrap_or(self.cycle_path.last().unwrap())
                .to_str()
                .expect("Couldn't process path string"),
        ))?;
        Ok(())
    }
}
impl Debug for DependencyCycle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for step in self.cycle_path.iter().take(self.cycle_path.len() - 1) {
            f.write_fmt(format_args!(
                "    {0} -->\n",
                step.strip_prefix(&self.root)
                    .unwrap_or(step)
                    .to_str()
                    .expect("Couldn't process path string")
            ))?;
        }
        f.write_fmt(format_args!(
            "    {0} --> {1}",
            self.cycle_path
                .last()
                .expect("Path is nonempty")
                .strip_prefix(&self.root)
                .unwrap_or(self.cycle_path.last().unwrap())
                .to_str()
                .expect("Couldn't process path string"),
            self.cycle_path
                .first()
                .expect("Path is nonempty")
                .strip_prefix(&self.root)
                .unwrap_or(self.cycle_path.first().unwrap())
                .to_str()
                .expect("Couldn't process path string"),
        ))?;

        Ok(())
    }
}
impl error::Error for DependencyCycle {}

#[derive(Debug, Clone)]
pub struct BuildArtifact {
    scripts: Vec<PathBuf>,
    version: Version,
    source_dir: PathBuf,
    title: String,
}
impl BuildArtifact {
    pub fn new(scripts: Vec<PathBuf>, info: &ProjectInfo) -> Self {
        Self {
            scripts,
            version: info.project.version.clone(),
            source_dir: info.source_dir(),
            title: info.project.title.clone(),
        }
    }
    pub fn set_version(&mut self, version: &Version) {
        self.version = version.clone();
    }
}
impl Artifact for BuildArtifact {
    fn compatible(&self, version: &Version) -> bool {
        // A build is a migration from 0.0.0
        let req = from_empty_database();
        req.matches(version)
    }
    fn version(&self) -> &Version {
        &self.version
    }
    fn spec(&self) -> (semver::VersionReq, Version) {
        (from_empty_database(), self.version.clone())
    }

    fn scripts<Consumer: ScriptConsumer>(
        &self,
        mut consumer: Consumer,
    ) -> Result<ContentId, ScriptProcessingError<Consumer::Error>> {
        let mut hasher = Sha256::new();
        let mut batch_buffer = Vec::<u8>::with_capacity(1024);
        let mut read_buffer = Vec::<u8>::with_capacity(1024);

        write!(
            batch_buffer,
            "-- [ {} {} ]\n\n",
            self.title.trim_ascii(),
            self.version
        )?;
        let batch = from_utf8(&batch_buffer)?;
        hasher.update(batch);
        consumer.accept(batch)?;

        let last_idx = self.scripts.len().saturating_sub(1);
        for (idx, script) in self.scripts.iter().enumerate() {
            batch_buffer.clear();
            read_buffer.clear();

            write!(
                batch_buffer,
                "-- [ {} ]\n\n",
                script.strip_prefix(&self.source_dir)?.to_str().unwrap()
            )?;

            let mut f = File::open(script)?;
            f.read_to_end(&mut read_buffer)?;

            batch_buffer.write_all(read_buffer.trim_ascii())?;
            if idx != last_idx {
                batch_buffer.write_all(b"\n\n")?;
            } else {
                batch_buffer.write_all(b"\n")?;
            }

            let batch = from_utf8(&batch_buffer)?;
            hasher.update(batch);
            consumer.accept(batch)?;
        }

        let id = hasher.finalize().into();
        consumer.commit(id)?;
        Ok(id)
    }
}
