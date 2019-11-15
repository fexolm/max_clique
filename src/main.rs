#[macro_use]
extern crate lazy_static;
extern crate regex;
extern crate time;

use std::io::Result;

mod graph;

fn print_clique(v: &Vec<u16>) {
    for &n in v {
        print!("{} ", n);
    }
}

fn main() -> Result<()> {
    use graph::Graph;
    use time::PreciseTime;

    // yes, hardcoded string here
    let graph = Graph::read(r"C:\Users\artem\Downloads\brock400_2.clq.txt")?;

    let start = PreciseTime::now();
    let max_clique = graph::get_max_clique(graph.clone());
    let end = PreciseTime::now();

    let spent = start.to(end);
    let sec = spent.num_seconds() - 60 * spent.num_minutes();
    let min = spent.num_minutes() - 60 * spent.num_hours();
    let hours = spent.num_hours();
    println!("time spent: {}h {}m {}s", hours, min, sec);
    println!("Max clique size: {}", max_clique.len());
    print_clique(&max_clique);
    Ok(())
}
