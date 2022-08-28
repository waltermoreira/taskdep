use anyhow::{anyhow, bail, Context, Result};
use clap::Parser;
use petgraph::graph::NodeIndex;
use petgraph::stable_graph::DefaultIx;
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

fn build_graph<R>(
    f: R,
    prefix: &[String],
    nodes: &mut HashMap<String, NodeIndex<DefaultIx>>,
    graph: &mut DiGraph<Node, String>,
) -> Result<()>
where
    R: Read,
{
    let yaml: HashMap<String, Value> = serde_yaml::from_reader(f)?;
    if let Some(incs) = yaml.get("includes") {
        let namespaces = incs
            .as_mapping()
            .ok_or_else(|| anyhow!("includes is not a mapping"))?;
        for (namespace, descr) in namespaces {
            let name = namespace
                .as_str()
                .ok_or_else(|| anyhow!("namespace is not a string"))?;
            let taskfile = match descr {
                Value::String(s) => s,
                Value::Mapping(m) => {
                    m.get("taskfile").and_then(|t| t.as_str()).ok_or_else(
                        || anyhow!("couldn't find taskfile name to include"),
                    )?
                }
                _ => bail!("incorrect type for an include"),
            };
            let f = File::open(taskfile)?;
            build_graph(f, &[prefix, &[name.into()]].concat(), nodes, graph)?;
        }
    }
    let tasks = yaml
        .get("tasks")
        .ok_or_else(|| anyhow!("tasks not found"))?
        .as_mapping()
        .ok_or_else(|| anyhow!("tasks is not a mapping"))?;
    for (task, descr) in tasks {
        let name = task
            .as_str()
            .ok_or_else(|| anyhow!("task name is not a string"))?;
        let name = [prefix, &[name.into()]].concat().join(":");
        nodes
            .entry(name.clone())
            .or_insert_with(|| graph.add_node(Node(name.clone())));
        if let Some(deps) = descr
            .as_mapping()
            .ok_or_else(|| anyhow!("task is not a mapping"))?
            .get("deps")
        {
            for dep in deps
                .as_sequence()
                .ok_or_else(|| anyhow!("deps is not a list"))?
            {
                let dep_name = match dep {
                    Value::String(n) => n,
                    Value::Mapping(m) => m
                        .get("task")
                        .and_then(|t| t.as_str())
                        .ok_or_else(|| anyhow!("couldn't find name of task"))?,
                    _ => bail!("incorrect type for a dependency"),
                };
                let full_dep_name =
                    [prefix, &[dep_name.into()]].concat().join(":");
                nodes.entry(full_dep_name.clone()).or_insert_with(|| {
                    graph.add_node(Node(full_dep_name.clone()))
                });
                graph.add_edge(
                    nodes[&full_dep_name],
                    nodes[&name],
                    format!("{full_dep_name}-{name}"),
                );
            }
        }
    }
    Ok(())
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
        .spawn()
        .context("command `dot` not found (please, make sure `graphviz` is installed)")?;
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
    let mut nodes = HashMap::new();
    let mut graph: DiGraph<Node, _> = DiGraph::new();
    build_graph(taskfile, &[], &mut nodes, &mut graph)?;
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
    use crate::{build_graph, graph_to_image};
    use indoc::indoc;
    use petgraph::prelude::DiGraph;
    use std::{
        collections::HashMap,
        fs::File,
        io::{Cursor, Result, Write},
    };

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
        let mut n = HashMap::new();
        let mut g = DiGraph::new();
        build_graph(yaml, &["foo".into()], &mut n, &mut g).unwrap();
        assert_eq!(g.node_count(), 5);
        assert_eq!(g.edge_count(), 4);
        Ok(())
    }

    #[test]
    fn test_build_graph_with_includes() -> Result<()> {
        let inc1 = indoc! {r#"
            tasks:
              task1_inc1:
                deps:
                    - task2_inc1
              task2_inc1:
                descr: descr
        "#};
        let inc2 = indoc! {r#"
            tasks:
              task1_inc2:
                foo: 1
        "#};
        let mut inc1_file = File::create("/tmp/inc1.yaml")?;
        write!(&mut inc1_file, "{}", inc1)?;
        let mut inc2_file = File::create("/tmp/inc2.yaml")?;
        write!(&mut inc2_file, "{}", inc2)?;
        let yaml = Cursor::new(String::from(indoc! {r#"
             foo: 1
             includes:
               inc1: /tmp/inc1.yaml
               inc2:
                 taskfile: /tmp/inc2.yaml
             tasks:
               foo:
                 deps:
                   - bar
                   - baz
                   - inc1:task1_inc1
                   - inc2:task1_inc2
            "#}));
        let mut n = HashMap::new();
        let mut g = DiGraph::new();
        build_graph(yaml, &[], &mut n, &mut g).unwrap();
        let i = graph_to_image(&g).unwrap();
        let mut out = File::create("/tmp/out.svg")?;
        out.write_all(&i.stdout)?;
        assert_eq!(g.node_count(), 6);
        assert_eq!(g.edge_count(), 5);
        Ok(())
    }
}
