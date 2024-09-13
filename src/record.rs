use regex::Regex;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    fmt::Debug,
    hash::Hash,
};

/// Vec<(is_unit, name)>.
/// A node can be either a unit name or a intermediate signal name.
pub type NameList = Vec<(bool, &'static str)>;

// return (receiver, producier)
pub type Updater<'a, UnitIn, UnitOut, Inter> =
    Box<&'a mut dyn FnMut(&mut UnitIn, &mut Inter, &mut Tracer, UnitOut)>;

#[derive(Debug)]
pub struct Graph {
    pub(crate) order: NameList,
    // Node format:
    // - `i.[fdemw].*`: stage register passin node
    // - `o.[fdemw].*`: stage register passout node
    // - `*`: intermediate signal node
    // - `*.*`: regular device input/output node
    // pub(crate) nodes: Vec<String>,
    // pub(crate) edges: Vec<(String, String)>,
}

fn replace_abbr(abbrs: &[(&'static str, &'static str)], str: &str) -> String {
    let mut r = str.to_string();
    for (origin, abbr) in abbrs {
        let re = Regex::new(&format!(r"\b{abbr}\b")).unwrap();
        r = re.replace_all(&r, *origin).to_string();
    }
    r
}
// impl Graph {
// pub(crate) fn get_node_index(&self, name: &str) -> Option<usize> {
//     let name = replace_abbr(&self.abbrs, name);
//     let r = self.nodes.iter().position(|u| u.contains(&name));
//     if r.is_some() {
//         return r;
//     }
//     let r = self.nodes.iter().position(|u| name.contains(u));
//     if r.is_some() {
//         return r;
//     }
//     // eprintln!("x: {}", name);
//     r
// }
// }

/// Compute topological order of nodes using BFS.
///
/// Return node list in order and their levels
pub fn topo<Node: Copy + Eq + Hash + Debug>(
    nodes: impl Iterator<Item = Node> + Clone,
    edges: impl Iterator<Item = (Node, Node)> + Clone,
) -> Vec<(Node, i32)> {
    let mut degree_level: HashMap<Node, (i32, i32)> = HashMap::default();
    for (_, to) in edges.clone() {
        let entry = degree_level.entry(to).or_default();
        entry.0 += 1;
    }
    let mut que: VecDeque<Node> = VecDeque::new();
    let mut levels = Vec::new();
    for node in nodes {
        if degree_level.get(&node).cloned().unwrap_or_default().0 == 0 {
            que.push_back(node)
        }
    }
    while let Some(head) = que.pop_front() {
        let level = degree_level.remove(&head).map(|o| o.1).unwrap_or(0);
        levels.push((head, level));
        // let mut is_depended = false;
        for (from, to) in edges.clone() {
            if from == head {
                // is_depended = true;
                let entry = degree_level.get_mut(&to).unwrap();
                entry.0 -= 1;
                entry.1 = entry.1.max(level + 1);
                if entry.0 == 0 {
                    que.push_back(to);
                }
            }
        }
    }

    if !degree_level.is_empty() {
        panic!("not DAG, degrees: {:?}", degree_level)
    }

    levels
}
pub struct GraphBuilder {
    runnable_nodes: NameList,
    unit_nodes: Vec<String>,
    nodes: BTreeSet<String>,
    edges: Vec<(String, String)>,
    stage_units: BTreeSet<&'static str>,
    /// (name, body)
    deps: Vec<(String, String)>,
    rev_deps: Vec<(String, String)>,
    // abbrs for pass output
    abbrs: Vec<(&'static str, &'static str)>,
    output_prefix: &'static str,
    input_prefix: &'static str,
}

impl GraphBuilder {
    pub fn new(output_prefix: &'static str, input_prefix: &'static str) -> Self {
        Self {
            nodes: Default::default(),
            runnable_nodes: Default::default(),
            unit_nodes: Default::default(),
            deps: Default::default(),
            rev_deps: Default::default(),
            edges: Default::default(),
            stage_units: Default::default(),
            abbrs: Default::default(),
            output_prefix,
            input_prefix,
        }
    }
    pub fn add_stage_output(&mut self, origin: &'static str, abbr: &'static str) {
        self.abbrs.push((origin, abbr));
    }
    fn add_edge(&mut self, from: String, to: String) {
        self.nodes.insert(from.clone());
        self.nodes.insert(to.clone());
        self.edges.push((from, to));
    }
    /// Unit `name` is the dependency of units exists in `body`.
    pub fn add_rev_deps(&mut self, name: &'static str, body: &'static str) {
        self.rev_deps.push((name.to_string(), body.to_string()))
    }
    pub fn add_unit_node(&mut self, unit_name: &'static str) {
        self.runnable_nodes.push((true, unit_name));
        self.unit_nodes.push(unit_name.to_string());
    }
    pub fn add_unit_input(&mut self, unit_name: &'static str, field_name: &'static str) {
        let full_name = String::from(unit_name) + "." + field_name;
        self.add_edge(full_name.to_string(), unit_name.to_string());
    }
    pub fn add_unit_output(&mut self, unit_name: &'static str, field_name: &'static str) {
        let full_name = String::from(unit_name) + "." + field_name;
        self.add_edge(unit_name.to_string(), full_name.to_string());
    }
    /// Stage units pass the current input data to the next cycle.
    /// These units should be run at the end.
    pub fn add_unit_stage(&mut self, unit_name: &'static str, field_name: &'static str) {
        self.nodes
            .insert(String::from(self.output_prefix) + "." + unit_name + "." + field_name);
        self.nodes
            .insert(String::from(self.input_prefix) + "." + unit_name + "." + field_name);
        // link input to unit, without output
        self.add_edge(
            String::from(self.input_prefix) + "." + unit_name + "." + field_name,
            unit_name.to_string(),
        );
        self.stage_units.insert(unit_name);
    }
    /// Set unit `name` as runnable, which depends on other units in `body`.
    pub fn add_update(&mut self, name: &'static str, body: &'static str) {
        self.runnable_nodes.push((false, name));
        self.nodes.insert(name.to_string());
        self.deps.push((name.to_string(), body.to_string()));
    }
    fn init_deps(&mut self) {
        // (from, to)
        let mut new_edges = Vec::new();
        for (name, body) in &self.deps {
            let body = replace_abbr(&self.abbrs, body);
            for node in &self.nodes {
                // edges from device to there inputs/outputs has already bean added
                // thus is filtered out
                if node != name && body.contains(node) && !self.unit_nodes.contains(node) {
                    new_edges.push((node.to_string(), name.to_string()));
                }
            }
        }
        for (name, body) in &self.rev_deps {
            let body = replace_abbr(&self.abbrs, body);
            for node in &self.nodes {
                // edges from device to there inputs/outputs has already bean added
                // thus is filtered out
                if node != name && body.contains(node) && !self.unit_nodes.contains(node) {
                    new_edges.push((name.to_string(), node.to_string()));
                }
            }
        }
        for (from, to) in new_edges {
            self.add_edge(from, to)
        }
    }
    /// Compute topological order of nodes.
    pub fn build(mut self) -> Graph {
        self.init_deps();

        let levels = topo(self.nodes.iter(), self.edges.iter().map(|(a, b)| (a, b)));
        let order: Vec<(bool, &'static str)> = levels
            .iter()
            .filter_map(|(node, _)| self.runnable_nodes.iter().find(|(_, p)| p == node).copied())
            .collect();

        let (mut last, mut order): (NameList, _) = order
            .into_iter()
            .partition(|o| o.0 && self.stage_units.contains(o.1));

        // put stage units at the end
        order.append(&mut last);

        // order
        Graph { order }
    }
}

#[derive(Default, Debug)]
pub struct Tracer {
    pub(crate) tunnel: Vec<&'static str>,
}
impl Tracer {
    pub fn trigger_tunnel(&mut self, name: &'static str) {
        if self.tunnel.contains(&name) {
            return;
        }
        self.tunnel.push(name);
    }
}

pub struct Record<'a, UnitIn, UnitOut, Inter> {
    unit_in: &'a mut UnitIn,
    context: &'a mut Inter,
    unit_out: UnitOut,
    updates: BTreeMap<&'static str, Updater<'a, UnitIn, UnitOut, Inter>>,
    tracer: Tracer,
}

impl<'a, UnitIn: Clone, UnitOut: Clone, Inter> Record<'a, UnitIn, UnitOut, Inter> {
    pub fn new(unit_in: &'a mut UnitIn, unit_out: UnitOut, context: &'a mut Inter) -> Self {
        Record {
            unit_in,
            unit_out,
            context,
            updates: Default::default(),
            tracer: Default::default(),
        }
    }
    /// Generally, a circuit update function accepts output signal from previous units,
    /// and then emits input signals of the next units or update intermediate signals.
    pub fn add_update(
        &mut self,
        name: &'static str,
        func: &'a mut impl FnMut(&mut UnitIn, &mut Inter, &mut Tracer, UnitOut),
    ) {
        self.updates.insert(name, Box::new(func));
    }
    /// Execute a combinatorial logic curcuits. See [`Record::add_update`].
    pub fn run_combinatorial_logic(&mut self, name: &'static str) {
        if let Some(func) = self.updates.get_mut(name) {
            func(
                self.unit_in,
                self.context,
                &mut self.tracer,
                self.unit_out.clone(),
            )
        } else {
            panic!("invalid name")
        }
    }
    /// Get current signals.
    pub fn signals(&self) -> (UnitIn, UnitOut) {
        (self.unit_in.clone(), self.unit_out.clone())
    }
    /// Update signals from outputs of a unit.
    pub fn update_from_unit_out(&mut self, unit_out: UnitOut) {
        self.unit_out = unit_out
    }
    pub fn finalize(self) -> (UnitOut, Tracer) {
        (self.unit_out, self.tracer)
    }
}

#[cfg(test)]
mod tests {
    use crate::record::{Record, Tracer};

    #[test]
    fn test() {
        let mut a = 0u64;
        let mut x = ();
        let b = 2u64;
        let mut updater = |_: &mut (), a: &mut u64, _: &mut Tracer, _| {
            *a = b;
        };
        let mut updater2 = |_: &mut (), a: &mut u64, _: &mut Tracer, _| {
            *a = *a + b;
        };
        let mut rcd = Record::new(&mut x, (), &mut a);
        rcd.add_update("test", &mut updater);
        rcd.add_update("test2", &mut updater2);
        rcd.run_combinatorial_logic("test");
        rcd.run_combinatorial_logic("test2");
        println!("a = {}, b = {}", a, b);
    }
}
