use anyhow::{anyhow, bail, Context, Result};
use clap::Parser;
use petgraph::visit::EdgeRef;
use petgraph::{
    algo::tarjan_scc,
    dot::{Config, Dot},
    graph::DiGraph,
};
use serde_yaml::{self, Value};
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
    let task_section =
        &yaml.get("tasks".into()).ok_or(anyhow!("tasks not found"))?;
    let tasks = task_section
        .as_mapping()
        .ok_or(anyhow!("tasks section is not a mapping"))?;
    for (task, descr) in tasks {
        let name = task.as_str().ok_or(anyhow!("task name is not a string"))?;
        nodes
            .entry(name)
            .or_insert_with(|| graph.add_node(Node(name.into())));
        let descr = descr
            .as_mapping()
            .ok_or(anyhow!("task description is not a mapping"))?;
        if let Some(deps) = descr.get("deps") {
            let deps =
                deps.as_sequence().ok_or(anyhow!("deps is not a list"))?;
            for dep in deps {
                let dep_name = match dep {
                    Value::String(n) => n,
                    Value::Mapping(m) => m
                        .get("task")
                        .and_then(|t| t.as_str())
                        .ok_or(anyhow!("couldn't find name of task"))?,
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
                    "color=\"red\"".into()
                } else {
                    "".into()
                }
            },
            &|_g, (idx, _n)| {
                if comps.contains(&idx) {
                    "color=\"red\"".into()
                } else {
                    "".into()
                }
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
    let mut stdin = dot.stdin.take().ok_or(anyhow!("couldn't open stdin"))?;
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
struct Args {}

fn main() {
    let _args = Args::parse();
    println!("Hello, world!");
}

#[cfg(test)]
mod test {
    use crate::{build_graph, graph_to_image};
    use indoc::indoc;
    use std::{
        fs::File,
        io::{BufReader, Cursor, Result},
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
        let g = build_graph(yaml).unwrap();
        assert_eq!(g.node_count(), 5);
        assert_eq!(g.edge_count(), 4);
        Ok(())
    }

    #[test]
    fn test_foo() -> Result<()> {
        let f = File::open("Taskfile.yaml")?;
        let buf = BufReader::new(f);
        // let v: Value = serde_yaml::from_reader(buf).unwrap();
        // let x = v.get("tasks").unwrap();
        let y = build_graph(buf).unwrap();
        let _x = graph_to_image(&y);
        //dbg!(x);
        //let g = Dot::with_config(&y, &[Config::EdgeNoLabel]);
        //dbg!(g);
        // let o = File::create("/tmp/foograph").unwrap();
        // let mut buf = BufWriter::new(o);
        // graph_to_dot(&y, &mut buf).unwrap();
        // dbg!(is_cyclic_directed(&y));
        // let z = tarjan_scc(&y);
        // dbg!(z);
        Ok(())
    }
}
