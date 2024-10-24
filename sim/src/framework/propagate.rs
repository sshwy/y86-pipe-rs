use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt::{Debug, Write},
    hash::Hash,
};

use crate::framework::CpuCircuit;

#[derive(Debug)]
pub struct PropOrderItem {
    pub is_unit: bool,
    pub name: &'static str,
    pub level: u32,
}

/// Vec<(is_unit, name)>.
/// A node can be either a unit name or a intermediate signal name.
pub type NameList = Vec<(bool, &'static str)>;

#[derive(Debug, Default)]
pub struct PropOrder {
    pub(crate) order: Vec<PropOrderItem>,
    /// `max_dist` is the maximum number of hardware units in a cycle that has
    /// to be executed one after another. i.e. the length of the critical path.
    /// It is used to determine the CPU clock time. A severely pipelined CPU
    /// tends to have a small `max_dist`.
    pub(crate) max_dist: u32,
    /// Edges of the computational graph.
    pub(crate) edges: Vec<(String, String)>,
}

impl std::fmt::Display for PropOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "propagate order:")?;
        let s = crate::utils::B;
        for chunk in self.order.chunk_by(|a, b| a.level == b.level) {
            write!(f, "{s}lv.{}{s:#}:", chunk[0].level + 1)?;
            for item in chunk {
                let s = if item.is_unit {
                    crate::utils::GRNB
                } else {
                    crate::utils::GRAY
                };
                write!(f, " {s}{}{s:#}", item.name)?
            }
            f.write_char('\n')?;
        }
        Ok(())
    }
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
        panic!(
            "The computational graph of your CPU pipeline is not a DAG! degrees: {:?}",
            degree_level
        )
    }

    levels
}
pub struct PropOrderBuilder {
    runnable_nodes_set: HashSet<String>,
    /// Runnable nodes includes units and intermediate signals.
    runnable_nodes: NameList,
    nodes: HashSet<String>,
    edges: Vec<(String, String)>,
}

impl Default for PropOrderBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PropOrderBuilder {
    pub fn new() -> Self {
        Self {
            runnable_nodes_set: Default::default(),
            runnable_nodes: Default::default(),
            nodes: Default::default(),
            edges: Default::default(),
        }
    }

    pub fn add_edge(&mut self, from: String, to: String) {
        self.nodes.insert(from.clone());
        self.nodes.insert(to.clone());
        self.edges.push((from.clone(), to.clone()));
    }

    /// Set unit `name` as runnable
    pub fn add_unit_node(&mut self, unit_name: &'static str) {
        if !self.runnable_nodes_set.insert(unit_name.to_string()) {
            panic!("duplicate unit name: {}", unit_name)
        }
        self.runnable_nodes.push((true, unit_name));
    }

    pub fn add_unit_input(&mut self, unit_name: &'static str, field_name: &'static str) {
        let full_name = String::from(unit_name) + "." + field_name;
        self.add_edge(full_name.to_string(), unit_name.to_string());
    }

    pub fn add_unit_output(&mut self, unit_name: &'static str, field_name: &'static str) {
        let full_name = String::from(unit_name) + "." + field_name;
        self.add_edge(unit_name.to_string(), full_name.to_string());
    }

    pub fn add_intermediate(&mut self, name: &'static str) {
        if !self.runnable_nodes_set.insert(name.to_string()) {
            panic!("duplicate intermediate name: {}", name)
        }
        self.runnable_nodes.push((false, name));
        self.nodes.insert(name.to_string());
    }

    /// Compute topological order of nodes.
    pub fn build(mut self) -> PropOrder {
        // remove duplicates
        self.edges = std::mem::take(&mut self.edges)
            .into_iter()
            .collect::<HashSet<(String, String)>>()
            .into_iter()
            .collect::<Vec<(String, String)>>();

        self.edges.sort();
        // check if every input of units has at least one source
        let independent_unit_in = self
            .runnable_nodes
            .iter()
            .filter(|(is_unit, _)| *is_unit)
            .find_map(|(_, unit_name)| {
                self.edges
                    .iter()
                    .filter(|(_, to)| to == unit_name)
                    .find(|(unit_in, _)| self.edges.iter().all(|(_, to)| to != unit_in))
            });
        if let Some(e) = independent_unit_in {
            panic!("unit input {} has no source", e.0)
        }

        let levels = topo(self.nodes.iter(), self.edges.iter().map(|(a, b)| (a, b)));
        let order: Vec<(bool, &'static str)> = levels
            .iter()
            .filter_map(|node| self.runnable_nodes.iter().find(|(_, p)| p == node).copied())
            .collect();

        // compute distance of each node from the source
        let mut dist: HashMap<&str, u32> = HashMap::new();
        for node in levels {
            let is_unit = self.runnable_nodes.iter().any(|(is, p)| *is && p == node);
            for from in self
                .edges
                .iter()
                .filter_map(|(from, to)| (to == node).then_some(from))
            {
                let dist_from = dist.get(from.as_str()).copied().unwrap_or(0);
                let dist_node = dist.entry(node).or_default();
                *dist_node = (*dist_node).max(dist_from + is_unit as u32);
            }
        }
        let max_dist = dist.values().max().copied().unwrap_or_default() + 1;

        let mut order = order
            .into_iter()
            .map(|(is_unit, p)| PropOrderItem {
                is_unit,
                name: p,
                level: dist.get(p).copied().unwrap_or_default(),
            })
            .collect::<Vec<_>>();
        order.sort_by_key(|a| a.level);

        // order
        PropOrder {
            order,
            max_dist,
            edges: self.edges,
        }
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
pub type Updater<T> = Box<
    dyn FnMut(
        &mut <T as CpuCircuit>::UnitIn,
        &mut <T as CpuCircuit>::Inter,
        &mut <T as CpuCircuit>::StageState,
        &mut Tracer,
        &<T as CpuCircuit>::UnitOut,
        &<T as CpuCircuit>::StageState,
    ),
>;

pub struct PropUpdates<T: CpuCircuit> {
    pub(crate) updates: HashMap<&'static str, Updater<T>>,
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

/// Simulate the combinational logic circuits using update functions.
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
    /// Generally, a circuit update function accepts output signal from previous
    /// units, and then emits input signals of the next units or update
    /// intermediate signals.
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
