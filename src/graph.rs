use std::collections::*;
use std::fs::File;
use std::io::*;
use std::iter::*;
use std::sync::Arc;
use std::sync::RwLock;
use std::u64;

use rayon::{Scope};
use regex::Regex;

pub struct Graph {
    adj_list: HashMap<u16, HashSet<u16>>,
}

pub struct MaxCliqueData {
    max_clique: Vec<u16>,
    current_clique: Vec<u16>,
}

fn parse_line(reader: &mut BufReader<File>) -> Option<(u16, u16)> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"^e (\d+) (\d+)").unwrap();
    }
    loop {
        let mut text = String::new();
        match reader.read_line(&mut text) {
            Ok(size) if size > 0 => {
                if let Some(caps) = RE.captures(&text) {
                    return Some((caps.get(1).unwrap().as_str().parse::<u16>().unwrap(),
                                 caps.get(2).unwrap().as_str().parse::<u16>().unwrap()));
                }
            }
            _ => return None
        }
    }
}

macro_rules! get_entry {
    ($map:expr, $key:expr) => (*($map.entry($key).or_insert(HashSet::new())))
}

impl Graph {
    pub fn read(filename: &str) -> Result<Arc<Self>> {
        let mut adj_list: HashMap<u16, HashSet<u16>> = HashMap::new();
        let file = File::open(filename)?;
        let mut reader = BufReader::new(file);
        while let Some((from, to)) = parse_line(&mut reader) {
            get_entry!(adj_list, from).insert(to);
            get_entry!(adj_list, to).insert(from);
        }
        Ok(Arc::new(Graph { adj_list, }))
    }

    fn degree(&self, node: u16) -> u16 {
        self.neighbours(node).len() as u16
    }

    fn neighbours(&self, node: u16) -> &HashSet<u16> {
        &self.adj_list[&node]
    }

    fn subgraph_neighbours<'i>(&'i self, subgraph: &'i HashSet<u16>, node: u16) -> impl Iterator<Item=&'i u16> {
        self.neighbours(node).intersection(subgraph)
    }

    fn clique_heuristic(&self, data: &mut MaxCliqueData, mut vertexes: HashSet<u16>) {
        if vertexes.is_empty() {
            if data.current_clique.len() > data.max_clique.len() {
                data.max_clique = data.current_clique.clone();
            }
            return
        }

        let best_vertex = vertexes.iter().copied().max_by_key(
            |v| self.subgraph_neighbours(&vertexes, *v).count()).unwrap();

        let neighbours = HashSet::from_iter(
            self.subgraph_neighbours(&vertexes, best_vertex).copied()
                .filter(|n| self.degree(*n) >= data.max_clique.len() as u16));

        vertexes.remove(&best_vertex);
        data.current_clique.push(best_vertex);
        self.clique_heuristic(data, neighbours);
        data.current_clique.pop();
    }

    fn max_clique_heuristic(&self, data: &mut MaxCliqueData) {
        let mut queue = BinaryHeap::from_iter(
            self.adj_list.keys().copied().map(|n| (self.degree(n), n)));

        while let Some((_, node)) = queue.pop() {
            if self.degree(node) > data.max_clique.len() as u16 {
                data.current_clique.push(node);
                self.clique_heuristic(data, HashSet::from_iter(
                    self.neighbours(node).iter().copied()
                        .filter(|n| self.degree(*n) > data.max_clique.len() as u16)
                ));
                data.current_clique.pop();
            }
        }
    }

    fn greedy_coloring(&self, vertexes: &HashSet<u16>) -> HashMap<u16, i16> {
        let mut res = HashMap::new();

        let mut powers = Vec::from_iter(
            vertexes.iter().copied()
                .map(|v| (v, Vec::from_iter(self.subgraph_neighbours(vertexes, v))))
        );
        powers.sort_unstable_by_key(|(_, v)| -(v.len() as i32));
        // works up to 1024 elements.
        // we wouldn't have move as if it will take too much time
        let mut used = [0; 16];

        let use_col = |arr: &mut [u64], c: i16| {
            arr[(c / 64) as usize] |= 1u64 << (c % 64) as u64;
        };

        let min_col = |arr: &[u64]| {
            for i in 0..16 {
                if arr[i] != !0u64 {
                    return (64 * i + (!arr[i]).trailing_zeros() as usize) as i16;
                }
            }
            unreachable!()
        };
        for (node, neighbours) in powers {
            for neighbour in neighbours {
                if let Some(&val) = res.get(neighbour) {
                    use_col(&mut used, val);
                }
            }
            res.insert(node, min_col(&used));
            used = [0; 16];
        }
        res
    }
}

fn max_clique_impl(graph: Arc<Graph>, max_clique: Arc<RwLock<Vec<u16>>>,
                   current_clique: &mut Vec<u16>,
                   mut vertexes: HashSet<u16>,
                   s: &Scope) {
    {
        let len = max_clique.read().unwrap().len();
        if current_clique.len() > len {
            println!("New max len: {}", len);
            *max_clique.write().unwrap() = current_clique.clone();
        }
    }

    let coloring = graph.greedy_coloring(&vertexes);
    let mut candidates = Vec::from_iter(coloring.iter());

    candidates.sort_unstable_by_key(|(_, &c)| -c);
    for (&v, &c) in candidates {
        {
            if current_clique.len() + c as usize + 1 <= max_clique.read().unwrap().len() {
                return;
            }
        }

        vertexes.remove(&v);
        let neighbours = HashSet::from_iter(
            graph.subgraph_neighbours(&vertexes, v).copied());

        // TODO use persistent stack
        let mut cur_clique = current_clique.clone();
        let g = graph.clone();
        let mc = max_clique.clone();

        s.spawn(move |sc| {
            cur_clique.push(v);
            max_clique_impl(g, mc, &mut cur_clique, neighbours, sc);
            cur_clique.pop();
        });
    }
}

pub fn get_max_clique(graph: Arc<Graph>) -> Vec<u16> {
    let mut data = MaxCliqueData { max_clique: vec!(), current_clique: vec!() };
    graph.max_clique_heuristic(&mut data);
    println!("Heuristic best: {}", data.max_clique.len());
    let max_clique = Arc::new(RwLock::new(data.max_clique));

    rayon::scope(|s| {
        max_clique_impl(graph.clone(), max_clique.clone(), &mut vec!(),
                        HashSet::from_iter(graph.adj_list.keys().cloned()), s);
    });

    let res = max_clique.read().unwrap();
    res.clone()
}
