//! Verification of the sync core: bounded exhaustive exploration of reachable states, plus seeded
//! simulation with an unreliable network.
//!
//! The exploration enumerates every state two replicas can reach through a fixed menu of operations,
//! up to a depth bound, and checks on every one of them that merge is commutative, associative and
//! idempotent, that every operation only moves a replica upward, and that two replicas converge after
//! exchanging state. The simulation then runs five replicas through long random histories with lost,
//! duplicated and reordered deliveries. Test code: panics are how failures surface.
#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout,
    // a, b, c, i, j, k are the notation of the lattice laws being checked
    clippy::many_single_char_names
)]

use sov_sim::Rng;
use sov_sync::{wire, Conflict, Doc, EntryKind, Replica, ReplicaId};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Clone)]
struct Sys {
    r: [Replica; 2],
}

fn key(s: &Sys) -> String {
    format!("{:?}|{:?}", s.r[0], s.r[1])
}

/// Applies operation `op` to replica `i`; `None` when the operation is not enabled.
fn step(s: &Sys, i: usize, op: usize) -> Option<Sys> {
    let mut n = s.clone();
    let j = 1 - i;
    match op {
        0 => n.r[i].set_field("f", "a"),
        1 => n.r[i].set_field("f", "b"),
        2 => {
            n.r[i].advance().ok()?;
        }
        3 => {
            n.r[i].reopen().ok()?;
        }
        4 => {
            n.r[i].add_entry(EntryKind::Labor, 1, "x");
        }
        5 => {
            let id = n.r[i].doc().live_entries().first().map(|e| e.id)?;
            n.r[i].void_entry(id).ok()?;
        }
        6 => {
            let other = s.r[j].doc().clone();
            n.r[i].receive(&other);
        }
        _ => unreachable!(),
    }
    Some(n)
}

struct Explored {
    docs: Vec<Doc>,
    states: usize,
    transitions: usize,
}

fn explore(max_depth: usize, max_states: usize) -> Explored {
    let start = Sys {
        r: [Replica::new(ReplicaId(1)), Replica::new(ReplicaId(2))],
    };
    let mut seen: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(Sys, usize)> = VecDeque::new();
    let mut docs: HashMap<String, Doc> = HashMap::new();
    seen.insert(key(&start));
    queue.push_back((start, 0));
    let mut transitions = 0;
    while let Some((s, depth)) = queue.pop_front() {
        for r in &s.r {
            docs.entry(wire::encode(r.doc()))
                .or_insert_with(|| r.doc().clone());
        }
        // Two replicas that exchange state converge, from every reachable state.
        let (mut x, mut y) = (s.r[0].doc().clone(), s.r[1].doc().clone());
        let (x0, y0) = (x.clone(), y.clone());
        x.merge(&y0);
        y.merge(&x0);
        assert_eq!(x, y, "replicas did not converge from a reachable state");
        assert!(x0.le(&x) && y0.le(&x), "merge is an upper bound");
        if depth == max_depth || seen.len() >= max_states {
            continue;
        }
        for i in 0..2 {
            for op in 0..7 {
                let Some(n) = step(&s, i, op) else { continue };
                transitions += 1;
                assert!(
                    s.r[i].doc().le(n.r[i].doc()),
                    "operation {op} on replica {i} moved a replica downward"
                );
                assert!(
                    s.r[1 - i].doc() == n.r[1 - i].doc(),
                    "an operation changed the other replica"
                );
                if seen.insert(key(&n)) {
                    queue.push_back((n, depth + 1));
                }
            }
        }
    }
    Explored {
        docs: docs.into_values().collect(),
        states: seen.len(),
        transitions,
    }
}

fn merged(a: &Doc, b: &Doc) -> Doc {
    let mut m = a.clone();
    m.merge(b);
    m
}

#[test]
fn lattice_laws_hold_on_every_reachable_state() {
    let e = explore(5, 6_000);
    println!(
        "explored {} system states, {} transitions, {} distinct documents",
        e.states,
        e.transitions,
        e.docs.len()
    );
    assert!(
        e.docs.len() > 150,
        "the exploration must be rich enough to mean something: {}",
        e.docs.len()
    );
    let mut domain: Vec<&Doc> = e.docs.iter().collect();
    domain.sort_by_key(|d| wire::encode(d));
    let stride = domain.len().div_ceil(300).max(1);
    let domain: Vec<&Doc> = domain.into_iter().step_by(stride).collect();
    println!("pairwise domain: {} documents", domain.len());
    for a in &e.docs {
        assert_eq!(merged(a, a), *a, "idempotent");
    }
    for a in &domain {
        for b in &domain {
            assert_eq!(merged(a, b), merged(b, a), "commutative");
        }
    }
    // Associativity: exhaustive when small, otherwise a large seeded sample of triples.
    let n = e.docs.len();
    let mut rng = Rng::new(7);
    let triples = if n <= 70 { n * n * n } else { 400_000 };
    for t in 0..triples {
        let (i, j, k) = if n <= 70 {
            (t / (n * n), (t / n) % n, t % n)
        } else {
            (pick(&mut rng, n), pick(&mut rng, n), pick(&mut rng, n))
        };
        let (a, b, c) = (&e.docs[i], &e.docs[j], &e.docs[k]);
        assert_eq!(
            merged(&merged(a, b), c),
            merged(a, &merged(b, c)),
            "associative"
        );
    }
}

fn pick(rng: &mut Rng, n: usize) -> usize {
    usize::try_from(rng.below(u64::try_from(n).unwrap())).unwrap()
}

#[test]
fn conflicts_are_a_function_of_state_alone() {
    let e = explore(4, 3_000);
    let mut with_conflicts = 0;
    for d in &e.docs {
        let round_trip = wire::decode(&wire::encode(d)).unwrap();
        assert_eq!(
            d.conflicts(),
            round_trip.conflicts(),
            "conflicts depend only on the replicated state"
        );
        with_conflicts += usize::from(!d.conflicts().is_empty());
    }
    assert!(
        with_conflicts > 20,
        "the exploration must reach conflicting states: {with_conflicts}"
    );
    let late = e.docs.iter().any(|d| {
        d.conflicts()
            .iter()
            .any(|c| matches!(c, Conflict::LateEntry { .. }))
    });
    let siblings = e.docs.iter().any(|d| {
        d.conflicts()
            .iter()
            .any(|c| matches!(c, Conflict::FieldSiblings { .. }))
    });
    assert!(late && siblings, "both conflict kinds are reachable");
}

/// A network that loses, duplicates and reorders messages.
struct Network {
    inbox: Vec<(usize, Doc)>,
}

fn simulate(seed: u64, replicas: usize, steps: usize) {
    let mut rng = Rng::new(seed);
    let mut nodes: Vec<Replica> = (0..replicas)
        .map(|i| Replica::new(ReplicaId(u16::try_from(i).unwrap() + 1)))
        .collect();
    let mut net = Network { inbox: Vec::new() };
    let mut created: Vec<sov_sync::EntryId> = Vec::new();
    let mut voided: Vec<sov_sync::EntryId> = Vec::new();
    for _ in 0..steps {
        let i = pick(&mut rng, replicas);
        match rng.below(10) {
            0 | 1 => nodes[i].set_field(
                ["a", "b", "c"][pick(&mut rng, 3)],
                &format!("v{}", rng.below(100)),
            ),
            2 => {
                let _ = nodes[i].advance();
            }
            3 => {
                if rng.chance(1, 5) {
                    let _ = nodes[i].reopen();
                }
            }
            4 | 5 => created.push(nodes[i].add_entry(
                if rng.chance(1, 2) {
                    EntryKind::Labor
                } else {
                    EntryKind::Material
                },
                rng.below(500),
                "n",
            )),
            6 => {
                if let Some(e) = nodes[i].doc().live_entries().first().map(|e| e.id) {
                    nodes[i].void_entry(e).unwrap();
                    voided.push(e);
                }
            }
            7 | 8 => {
                // Send: possibly lost, possibly duplicated.
                let to = pick(&mut rng, replicas);
                if rng.chance(7, 10) {
                    net.inbox.push((to, nodes[i].doc().clone()));
                    if rng.chance(1, 4) {
                        net.inbox.push((to, nodes[i].doc().clone()));
                    }
                }
            }
            _ => {
                // Deliver one pending message, chosen at random (reordering).
                if !net.inbox.is_empty() {
                    let k = pick(&mut rng, net.inbox.len());
                    let (to, doc) = net.inbox.swap_remove(k);
                    nodes[to].receive(&doc);
                }
            }
        }
    }
    // Before any repair the replicas may differ. After a full exchange they must all agree.
    for _ in 0..2 {
        for i in 0..replicas {
            for j in 0..replicas {
                let d = nodes[j].doc().clone();
                nodes[i].receive(&d);
            }
        }
    }
    for n in &nodes[1..] {
        assert_eq!(
            n.doc(),
            nodes[0].doc(),
            "seed {seed}: replicas diverged after full exchange"
        );
    }
    let final_doc = nodes[0].doc();
    let live: HashSet<_> = final_doc.all_entries().iter().map(|e| e.id).collect();
    for id in &created {
        assert!(live.contains(id), "seed {seed}: entry {id:?} was lost");
    }
    for id in &voided {
        assert!(
            final_doc.is_void(*id),
            "seed {seed}: void of {id:?} was undone"
        );
    }
    assert_eq!(
        wire::decode(&wire::encode(final_doc)).unwrap(),
        *final_doc,
        "seed {seed}: wire round trip"
    );
}

#[test]
fn five_replicas_converge_over_an_unreliable_network_without_losing_entries() {
    for seed in 0..300 {
        simulate(seed, 5, 400);
    }
}

#[test]
fn two_replicas_and_three_replicas_converge_too() {
    for seed in 1000..1100 {
        simulate(seed, 2, 300);
        simulate(seed, 3, 300);
    }
}
