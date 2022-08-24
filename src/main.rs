use anyhow::{anyhow, bail, Context, Result};
use clap::Parser;
use petgraph::visit::EdgeRef;
use petgraph::{
    algo::tarjan_scc,
    dot::{Config, Dot},
    graph::DiGraph,
};
use serde_yaml::{self, Value};
use std::fs::{canonicalize, File};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::{
    collections::{HashMap, HashSet},
    fmt::{self, Debug, Formatter},
    io::{Read, Write},
};

struct Node(String);

impl Debug for Node {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

fn build_graph<R>(f: R) -> Result<DiGraph<Node, String>>
where
    R: Read,
{
    let yaml: HashMap<String, Value> = serde_yaml::from_reader(f)?;
    let mut graph: DiGraph<Node, _> = DiGraph::new();
    let mut nodes = HashMap::new();
    let task_section = &yaml
        .get("tasks")
        .ok_or_else(|| anyhow!("tasks not found"))?;
    let tasks = task_section
        .as_mapping()
        .ok_or_else(|| anyhow!("tasks section is not a mapping"))?;
    for (task, descr) in tasks {
        let name = task
            .as_str()
            .ok_or_else(|| anyhow!("task name is not a string"))?;
        nodes
            .entry(name)
            .or_insert_with(|| graph.add_node(Node(name.into())));
        let descr = descr
            .as_mapping()
            .ok_or_else(|| anyhow!("task description is not a mapping"))?;
        if let Some(deps) = descr.get("deps") {
            let deps = deps
                .as_sequence()
                .ok_or_else(|| anyhow!("deps is not a list"))?;
            for dep in deps {
                let dep_name = match dep {
                    Value::String(n) => n,
                    Value::Mapping(m) => m
                        .get("task")
                        .and_then(|t| t.as_str())
                        .ok_or_else(|| anyhow!("couldn't find name of task"))?,
                    _ => bail!("incorrect type for a dependency"),
                };
                nodes
                    .entry(dep_name)
                    .or_insert_with(|| graph.add_node(Node(dep_name.into())));
                graph.add_edge(
                    nodes[dep_name],
                    nodes[name],
                    format!("{dep_name}:{name}"),
                );
            }
        }
    }
    Ok(graph)
}

fn graph_to_dot(g: &DiGraph<Node, String>) -> String {
    let components = tarjan_scc(&g);
    let comps = components
        .iter()
        .filter(|c| c.len() > 1)
        .flatten()
        .collect::<HashSet<_>>();
    format!(
        "{:?}",
        Dot::with_attr_getters(
            g,
            &[Config::EdgeNoLabel],
            &|_g, e| {
                if comps.contains(&e.source()) && comps.contains(&e.target()) {
                    "color=\"red\""
                } else {
                    ""
                }
                .into()
            },
            &|_g, (idx, _n)| {
                if comps.contains(&idx) {
                    "color=\"red\""
                } else {
                    ""
                }
                .into()
            }
        )
    )
}

fn graph_to_image(g: &DiGraph<Node, String>) -> Result<Output> {
    let contents = graph_to_dot(g);
    let mut dot = Command::new("dot")
        .arg("-Tsvg")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stdin = dot
        .stdin
        .take()
        .ok_or_else(|| anyhow!("couldn't open stdin"))?;
    let stdin_write = thread::spawn(move || {
        stdin
            .write_all(contents.as_bytes())
            .with_context(|| "couldn't write to stdin")
    });
    let run_dot = thread::spawn(move || {
        dot.wait_with_output().with_context(|| "couldn't run `dot`")
    });
    stdin_write
        .join()
        .map_err(|e| anyhow!("stdin_write: {e:?}"))??;
    run_dot.join().map_err(|e| anyhow!("run_dot: {e:?}"))?
}

#[derive(Parser, Debug)]
#[clap(version = env!("CARGO_PKG_VERSION"))]
#[clap(name = "taskdep")]
#[clap(term_width = 80)]
/// Display Taskfile dependency graph
///
/// Consume `Taskfile.yaml` and generate `Taskfile.svg` showing the dependency graph.
/// Cycles in the graph show in color red.
struct Args {
    /// Do not open browser with the image file
    #[clap(short, long, action)]
    silent: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let taskfile = File::open("Taskfile.yaml")
        .map_err(|e| anyhow!("Taskfile.yaml: {e}"))?;
    let graph = build_graph(taskfile)?;
    let image = graph_to_image(&graph)?;
    if !image.status.success() {
        bail!("failed to create image: {}", image.status);
    }
    let mut image_file = File::create("Taskfile.svg")?;
    image_file.write_all(&image.stdout)?;
    if !args.silent {
        let taskfile = canonicalize("Taskfile.svg")?;
        let url = format!("file://{}", taskfile.to_string_lossy());
        webbrowser::open(&url)?;
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use crate::build_graph;
    use indoc::indoc;
    use std::io::{Cursor, Result};

    #[test]
    fn test_build_graph() -> Result<()> {
        let yaml = Cursor::new(String::from(indoc! {r#"
             foo: 1
             tasks:
               foo:
                 desc: desc
                 deps:
                   - bar
                   - baz
               bar:
                 deps:
                   - task: spam
                     params: params
               baz:
                 deps:
                   - spam
               spam:
                 desc: spam
               eggs:
                 desc: no deps
            "#}));
        let g = build_graph(yaml).unwrap();
        assert_eq!(g.node_count(), 5);
        assert_eq!(g.edge_count(), 4);
        Ok(())
    }
}
