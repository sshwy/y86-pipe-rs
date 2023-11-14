use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

use regex::Regex;

/// Vec<(is_device, name)>
pub type NameList = Vec<(bool, &'static str)>;
pub type TransLog = Vec<(&'static str, &'static str)>;
// return (receiver, producier)
pub type Updater<'a, DevIn, DevOut, Inter> =
    Box<&'a mut dyn FnMut(&mut DevIn, &mut Inter, &mut TransLog, DevOut)>;

pub struct RecordBuilder<'a, DevIn, DevOut, Inter> {
    runnable_nodes: NameList,
    device_nodes: Vec<String>,
    nodes: BTreeSet<String>,
    edges: Vec<(String, String)>,
    passed_devices: BTreeSet<&'static str>,
    deps: Vec<(String, String)>,
    rev_deps: Vec<(String, String)>,
    // abbrs for pass output
    abbrs: Vec<(&'static str, &'static str)>,
    updates: BTreeMap<&'static str, Updater<'a, DevIn, DevOut, Inter>>,
    output_prefix: &'static str,
    input_prefix: &'static str,
    preserved_order: Option<NameList>
}

impl<'a, DevIn: Clone, DevOut: Clone, Inter> RecordBuilder<'a, DevIn, DevOut, Inter> {
    pub fn new(output_prefix: &'static str, input_prefix: &'static str, preserved_order: Option<NameList>) -> Self {
        Self {
            nodes: Default::default(),
            runnable_nodes: Default::default(),
            device_nodes: Default::default(),
            deps: Default::default(),
            rev_deps: Default::default(),
            edges: Default::default(),
            passed_devices: Default::default(),
            abbrs: Default::default(),
            updates: Default::default(),
            output_prefix,
            input_prefix,
            preserved_order,
        }
    }
    pub fn add_pass_output(&mut self, origin: &'static str, abbr: &'static str) {
        if self.preserved_order.is_some() {
            return
        }
        self.abbrs.push((origin, abbr));
    }
    fn add_edge(&mut self, from: String, to: String) {
        if self.preserved_order.is_some() {
            return
        }
        self.nodes.insert(from.clone());
        self.nodes.insert(to.clone());
        self.edges.push((from, to));
    }
    pub fn add_rev_deps(&mut self, name: &'static str, body: &'static str) {
        if self.preserved_order.is_some() {
            return
        }
        self.rev_deps.push((name.to_string(), body.to_string()))
    }
    pub fn add_device_node(&mut self, dev_name: &'static str) {
        if self.preserved_order.is_some() {
            return
        }
        self.runnable_nodes.push((true, dev_name));
        self.device_nodes.push(dev_name.to_string());
    }
    pub fn add_device_input(&mut self, dev_name: &'static str, field_name: &'static str) {
        if self.preserved_order.is_some() {
            return
        }
        self.add_edge(
            dev_name.to_string() + "." + field_name,
            dev_name.to_string(),
        );
    }
    pub fn add_device_output(&mut self, dev_name: &'static str, field_name: &'static str) {
        if self.preserved_order.is_some() {
            return
        }
        self.add_edge(
            dev_name.to_string(),
            dev_name.to_string() + "." + field_name,
        );
    }
    // pass means compute next input data from the current output data
    // these device should be run at the end
    pub fn add_device_pass(&mut self, dev_name: &'static str, field_name: &'static str) {
        if self.preserved_order.is_some() {
            return
        }
        self.nodes
            .insert(String::from(self.output_prefix) + "." + dev_name + "." + field_name);
        self.nodes
            .insert(String::from(self.input_prefix) + "." + dev_name + "." + field_name);
        // link input to device, without output
        self.add_edge(
            String::from(self.input_prefix) + "." + dev_name + "." + field_name,
            dev_name.to_string(),
        );
        self.passed_devices.insert(dev_name);
    }
    /// body: dependency
    pub fn add_update(
        &mut self,
        name: &'static str,
        body: &'static str,
        func: &'a mut impl FnMut(&mut DevIn, &mut Inter, &mut TransLog, DevOut),
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
                // edges from device to there inputs/outputs has already bean added
                // thus is filtered out
                if node != name && body.contains(node) && !self.device_nodes.contains(node) {
                    new_edges.push((node.to_string(), name.to_string()));
                }
            }
        }
        for (name, body) in &self.rev_deps {
            let body = self.replace_abbr(body);
            for node in &self.nodes {
                // edges from device to there inputs/outputs has already bean added
                // thus is filtered out
                if node != name && body.contains(node) && !self.device_nodes.contains(node) {
                    new_edges.push((name.to_string(), node.to_string()));
                }
            }
        }
        for (from, to) in new_edges {
            self.add_edge(from, to)
        }
    }
    pub fn toporder(&mut self) -> NameList {
        self.init_deps();

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
            if let Some(rnode) = self.runnable_nodes.iter().find(|(_, p)| p == head) {
                order.push(*rnode);
            }
            let mut is_depended = false;
            for (from, to) in &self.edges {
                // make sure stage registers are not depended
                assert!(!self.passed_devices.contains(from.as_str()));
                if from == head {
                    is_depended = true;
                    let entry = degree.get_mut(to).unwrap();
                    *entry -= 1;
                    if *entry == 0 {
                        que.push_back(to);
                    }
                }
            }
            if !is_depended {
                eprintln!("not depended: {}", head)
            }
        }

        eprintln!("edges: {:?}", &self.edges);

        if !degree.is_empty() {
            panic!("not DAG, degrees: {:?}", degree)
        }

        let (mut last, mut order): (NameList, _) = order
            .into_iter()
            .partition(|o| o.0 && self.passed_devices.contains(o.1));

        // put passed device (mainly stage registers) at the end
        order.append(&mut last);

        eprintln!("order: {:?}", &order);
        order
    }
    pub fn build(
        mut self,
        devin: &'a mut DevIn,
        devout: DevOut,
        context: &'a mut Inter,
    ) -> Record<'a, DevIn, DevOut, Inter> {
        let order = if let Some(order) = self.preserved_order {
            order
        } else {
            self.toporder()
        };
        Record {
            devin,
            devout,
            context,
            order,
            updates: self.updates,
        }
    }
}

pub struct Record<'a, DevIn, DevOut, Inter> {
    devin: &'a mut DevIn,
    context: &'a mut Inter,
    devout: DevOut,
    order: NameList,
    updates: BTreeMap<&'static str, Updater<'a, DevIn, DevOut, Inter>>,
}

impl<'a, DevIn: Clone, DevOut: Clone, Inter> Record<'a, DevIn, DevOut, Inter> {
    pub fn run_name(&mut self, name: &'static str, trace: &mut TransLog) {
        if let Some(func) = self.updates.get_mut(name) {
            func(self.devin, self.context, trace, self.devout.clone())
        } else {
            panic!("invalid name")
        }
    }
    pub fn clone_devsigs(&self) -> (DevIn, DevOut) {
        (self.devin.clone(), self.devout.clone())
    }
    pub fn update_devout(&mut self, devout: DevOut) {
        self.devout = devout
    }
    pub fn toporder(&self) -> NameList {
        self.order.clone()
    }
}

#[cfg(test)]
mod tests {
    use crate::record::{RecordBuilder, TransLog};

    #[test]
    fn test() {
        let mut a = 0u64;
        let mut x = ();
        let mut rcd = RecordBuilder::new("o", "i", None);
        let b = 2u64;
        let mut updater = |_: &mut (), a: &mut u64, _: &mut TransLog, _| {
            *a = b;
        };
        let mut updater2 = |_: &mut (), a: &mut u64, _: &mut TransLog, _| {
            *a = *a + b;
        };
        let mut logs = Vec::new();
        rcd.add_update("test", "", &mut updater);
        rcd.add_update("test2", "", &mut updater2);
        let mut rcd = rcd.build(&mut x, (), &mut a);
        rcd.run_name("test", &mut logs);
        rcd.run_name("test2", &mut logs);
        println!("a = {}, b = {}", a, b);
    }
}
