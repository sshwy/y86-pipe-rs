use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    fmt::Debug,
    hash::Hash,
};

use crate::framework::CpuCircuit;

/// Vec<(is_unit, name)>.
/// A node can be either a unit name or a intermediate signal name.
pub type NameList = Vec<(bool, &'static str)>;

#[derive(Debug, Default)]
pub struct PropOrder {
    pub(crate) order: NameList,
}

/// Compute topological order of nodes using BFS.
///
/// Return node list in order and their levels
pub fn topo<Node: Copy + Eq + Hash + Debug>(
    nodes: impl Iterator<Item = Node> + Clone,
    edges: impl Iterator<Item = (Node, Node)> + Clone,
) -> Vec<Node> {
    let mut degree_level: HashMap<Node, i32> = HashMap::default();
    for (_, to) in edges.clone() {
        let entry = degree_level.entry(to).or_default();
        *entry += 1;
    }
    let mut que: VecDeque<Node> = VecDeque::new();
    let mut levels = Vec::new();
    for node in nodes {
        if degree_level.get(&node).cloned().unwrap_or_default() == 0 {
            que.push_back(node)
        }
    }
    while let Some(head) = que.pop_front() {
        degree_level.remove(&head);
        levels.push(head);
        for (from, to) in edges.clone() {
            if from == head {
                let entry = degree_level.get_mut(&to).unwrap();
                *entry -= 1;
                if *entry == 0 {
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
pub struct PropOrderBuilder {
    runnable_nodes: NameList,
    unit_nodes: Vec<String>,
    nodes: BTreeSet<String>,
    edges: Vec<(String, String)>,
    /// (name, body)
    deps: Vec<(String, String)>,
    rev_deps: Vec<(String, String)>,
}

impl PropOrderBuilder {
    pub fn new() -> Self {
        Self {
            nodes: Default::default(),
            runnable_nodes: Default::default(),
            unit_nodes: Default::default(),
            deps: Default::default(),
            rev_deps: Default::default(),
            edges: Default::default(),
        }
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
    /// Set unit `name` as runnable
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
    pub fn add_update(&mut self, name: &'static str, body: &'static str) {
        self.runnable_nodes.push((false, name));
        self.nodes.insert(name.to_string());
        self.deps.push((name.to_string(), body.to_string()));
    }
    fn init_deps(&mut self) {
        // (from, to)
        let mut new_edges = Vec::new();
        for (name, body) in &self.deps {
            for node in &self.nodes {
                // edges from device to there inputs/outputs has already bean added
                // thus is filtered out
                if node != name && body.contains(node) && !self.unit_nodes.contains(node) {
                    new_edges.push((node.to_string(), name.to_string()));
                }
            }
        }
        for (name, body) in &self.rev_deps {
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
    pub fn build(mut self) -> PropOrder {
        self.init_deps();

        let levels = topo(self.nodes.iter(), self.edges.iter().map(|(a, b)| (a, b)));
        let order: Vec<(bool, &'static str)> = levels
            .iter()
            .filter_map(|node| self.runnable_nodes.iter().find(|(_, p)| p == node).copied())
            .collect();

        // order
        PropOrder { order }
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

// Update input and intermediate signals from output signals.
pub type Updater<UnitIn, UnitOut, Inter, StageState> =
    Box<dyn FnMut(&mut UnitIn, &mut Inter, &mut StageState, &mut Tracer, &UnitOut, &StageState)>;

pub struct PropUpdates<T: CpuCircuit> {
    pub(crate) updates:
        BTreeMap<&'static str, Updater<T::UnitIn, T::UnitOut, T::Inter, T::StageState>>,
}

impl<T: CpuCircuit> PropUpdates<T> {
    pub fn make_propagator<'a>(
        &'a mut self,
        unit_in: &'a mut T::UnitIn,
        unit_out: T::UnitOut,
        nex_state: &'a mut T::StageState,
        cur_state: &'a T::StageState,
        context: &'a mut T::Inter,
    ) -> Propagator<'a, T> {
        Propagator {
            unit_in,
            unit_out,
            nex_state,
            cur_state,
            context,
            updates: self,
            tracer: Default::default(),
        }
    }
}

/// Simulate the combinational logic circuits by update functions.
pub struct PropCircuit<T: CpuCircuit> {
    pub updates: PropUpdates<T>,
    pub order: PropOrder,
}

impl<T: CpuCircuit> PropCircuit<T> {
    pub fn new(order: PropOrder) -> Self {
        Self {
            updates: PropUpdates {
                updates: Default::default(),
            },
            order,
        }
    }
}

impl<T: CpuCircuit> PropCircuit<T> {
    /// Generally, a circuit update function accepts output signal from previous units,
    /// and then emits input signals of the next units or update intermediate signals.
    pub fn add_update(
        &mut self,
        name: &'static str,
        func: impl FnMut(
                &mut T::UnitIn,
                &mut T::Inter,
                &mut T::StageState,
                &mut Tracer,
                &T::UnitOut,
                &T::StageState,
            ) + 'static,
    ) {
        self.updates.updates.insert(name, Box::new(func));
    }
}

/// Propagator simulates the combinational logic circuits.
pub struct Propagator<'a, T: CpuCircuit> {
    unit_in: &'a mut T::UnitIn,
    unit_out: T::UnitOut,
    cur_state: &'a T::StageState,
    nex_state: &'a mut T::StageState,
    context: &'a mut T::Inter,
    updates: &'a mut PropUpdates<T>,
    tracer: Tracer,
}

impl<'a, T: CpuCircuit> Propagator<'a, T>
where
    T::UnitIn: Clone,
    T::UnitOut: Clone,
{
    /// Execute a combinatorial logic curcuits. See [`Propagator::add_update`].
    pub fn run_combinatorial_logic(&mut self, name: &'static str) {
        if let Some(func) = self.updates.updates.get_mut(name) {
            func(
                self.unit_in,
                self.context,
                self.nex_state,
                &mut self.tracer,
                &self.unit_out,
                self.cur_state,
            )
        } else {
            panic!("invalid name")
        }
    }
    /// Execute a unit.
    pub fn run_unit(&mut self, unit_fn: impl FnOnce(&T::UnitIn, &mut T::UnitOut)) {
        unit_fn(self.unit_in, &mut self.unit_out)
    }
    /// Get current signals.
    pub fn signals(&self) -> (T::UnitIn, T::UnitOut) {
        (self.unit_in.clone(), self.unit_out.clone())
    }
    pub fn finalize(self) -> (T::UnitOut, Tracer) {
        (self.unit_out, self.tracer)
    }
}
