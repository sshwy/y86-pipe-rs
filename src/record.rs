use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

use regex::Regex;

type Updater<'a, DevIn, Inter> = Box<&'a mut dyn FnMut(&mut DevIn, &mut Inter)>;

pub struct RecordBuilder<'a, DevIn, Inter> {
    runnable_nodes: Vec<(bool, &'static str)>,
    nodes: BTreeSet<String>,
    edges: Vec<(String, String)>,
    passed_devices: BTreeSet<&'static str>,
    deps: Vec<(String, String)>,
    rev_deps: Vec<(String, String)>,
    // abbrs for pass output
    abbrs: Vec<(&'static str, &'static str)>,
    updates: BTreeMap<&'static str, Updater<'a, DevIn, Inter>>,
    output_prefix: &'static str,
    input_prefix: &'static str,
}

impl<'a, DevIn: Clone, Inter> RecordBuilder<'a, DevIn, Inter> {
    pub fn new(output_prefix: &'static str, input_prefix: &'static str) -> Self {
        Self {
            nodes: Default::default(),
            runnable_nodes: Default::default(),
            deps: Default::default(),
            rev_deps: Default::default(),
            edges: Default::default(),
            passed_devices: Default::default(),
            abbrs: Default::default(),
            updates: Default::default(),
            output_prefix,
            input_prefix,
        }
    }
    pub fn add_pass_output(&mut self, origin: &'static str, abbr: &'static str) {
        self.abbrs.push((origin, abbr));
    }
    fn add_edge(&mut self, from: String, to: String) {
        self.nodes.insert(from.clone());
        self.nodes.insert(to.clone());
        self.edges.push((from, to));
    }
    pub fn add_rev_deps(&mut self, name: &'static str, body: &'static str) {
        self.rev_deps.push((name.to_string(), body.to_string()))
    }
    pub fn add_device_node(&mut self, dev_name: &'static str) {
        self.runnable_nodes.push((true, dev_name));
    }
    pub fn add_device_input(&mut self, dev_name: &'static str, field_name: &'static str) {
        self.add_edge(
            dev_name.to_string() + "." + field_name,
            dev_name.to_string(),
        );
    }
    pub fn add_device_output(&mut self, dev_name: &'static str, field_name: &'static str) {
        self.add_edge(
            dev_name.to_string(),
            dev_name.to_string() + "." + field_name,
        );
    }
    // pass means compute next input data from the current output data
    // these device should be run at the end
    pub fn add_device_pass(&mut self, dev_name: &'static str, field_name: &'static str) {
        self.nodes
            .insert(String::from(self.output_prefix) + "." + dev_name + "." + field_name);
        self.nodes
            .insert(String::from(self.input_prefix) + "." + dev_name + "." + field_name);
        self.passed_devices.insert(dev_name);
    }
    /// body: dependency
    pub fn add_update(
        &mut self,
        name: &'static str,
        body: &'static str,
        func: &'a mut impl FnMut(&mut DevIn, &mut Inter),
    ) {
        self.runnable_nodes.push((false, name));
        self.nodes.insert(name.to_string());
        self.deps.push((name.to_string(), body.to_string()));
        self.updates.insert(name, Box::new(func));
    }
    fn replace_abbr(&self, str: &str) -> String {
        let mut r = str.to_string();
        for (origin, abbr) in &self.abbrs {
            let re = Regex::new(&format!(r"\b{abbr}\b")).unwrap();
            r = re.replace_all(&r, *origin).to_string();
        }
        r
    }
    fn init_deps(&mut self) {
        let mut new_edges = Vec::new();
        for (name, body) in &self.deps {
            let body = self.replace_abbr(body);
            // dbg!(&body);
            for node in &self.nodes {
                if node != name && body.contains(node) {
                    new_edges.push((node.to_string(), name.to_string()));
                }
            }
        }
        for (name, body) in &self.rev_deps {
            let body = self.replace_abbr(body);
            for node in &self.nodes {
                if node != name && body.contains(node) {
                    new_edges.push((name.to_string(), node.to_string()));
                }
            }
        }
        for (from, to) in new_edges {
            self.add_edge(from, to)
        }
    }
    // return Vec<(is_device, name)>
    pub fn toporder(&mut self) -> Vec<(bool, &'static str)> {
        self.init_deps();
        dbg!(&self.nodes);

        let mut degree: HashMap<String, i32> = HashMap::default();
        for (_, to) in &self.edges {
            *degree.entry(to.to_string()).or_default() += 1;
        }
        let mut que = VecDeque::new();
        for node in &self.nodes {
            if degree.get(node).cloned().unwrap_or_default() == 0 {
                if !node.starts_with(self.output_prefix) {
                    eprintln!("zero degree: {}", node);
                }
                que.push_back(node)
            }
        }
        let mut order = Vec::new();
        while let Some(head) = que.pop_front() {
            degree.remove(head);
            if let Some(rnode) = self
                .runnable_nodes
                .iter()
                .find(|(_, p)| p == head)
            {
                order.push(*rnode);
            }
            for (from, to) in &self.edges {
                if from == head {
                    let entry = degree.get_mut(to).unwrap();
                    *entry -= 1;
                    if *entry == 0 {
                        que.push_back(to);
                    }
                }
            }
        }

        // dbg!(&self.edges);
        // dbg!(&self.deps);
        dbg!(&order);

        if !degree.is_empty() {
            panic!("not DAG, degrees: {:?}", degree)
        }

        order
    }
    pub fn build(
        mut self,
        devin: &'a mut DevIn,
        context: &'a mut Inter,
    ) -> Record<'a, DevIn, Inter> {
        Record {
            devin,
            context,
            order: self.toporder(),
            updates: self.updates,
        }
    }
}

pub struct Record<'a, DevIn, Inter> {
    devin: &'a mut DevIn,
    context: &'a mut Inter,
    order: Vec<(bool, &'static str)>,
    updates: BTreeMap<&'static str, Updater<'a, DevIn, Inter>>,
}

impl<'a, DevIn: Clone, Inter> Record<'a, DevIn, Inter> {
    pub fn run_name(&mut self, name: &'static str) {
        if let Some(func) = self.updates.get_mut(name) {
            func(self.devin, self.context)
        } else {
            panic!("invalid name")
        }
    }
    pub fn clone_devin(&self) -> DevIn {
        self.devin.clone()
    }
    pub fn toporder(&self) -> Vec<(bool, &'static str)> {
        self.order.clone()
    }
}

#[cfg(test)]
mod tests {
    use crate::record::RecordBuilder;

    #[test]
    fn test() {
        let mut a = 0u64;
        let mut x = ();
        let mut rcd = RecordBuilder::new("o", "i");
        let b = 2u64;
        let mut updater = |_: &mut (), a: &mut u64| {
            *a = b;
        };
        let mut updater2 = |_: &mut (), a: &mut u64| {
            *a = *a + b;
        };
        rcd.add_update("test", "", &mut updater);
        rcd.add_update("test2", "", &mut updater2);
        let mut rcd = rcd.build(&mut x, &mut a);
        rcd.run_name("test");
        rcd.run_name("test2");
        println!("a = {}, b = {}", a, b);
    }
}
