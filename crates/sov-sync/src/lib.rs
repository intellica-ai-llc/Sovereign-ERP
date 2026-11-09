//! Connectivity Fabric core (arc42 section 5, ADR eng-0011): offline-first replication of a work order.
//!
//! A mobile technician and a desk planner edit the same work order, possibly for hours without a
//! connection. This crate defines the work order as a **join-semilattice** (a state-based CRDT), so
//! that replicas which have seen the same changes hold the same state, in whatever order, however
//! many times, and through whatever intermediaries the changes arrived:
//!
//! - merge is **commutative, associative and idempotent**, and every local operation only moves a
//!   replica upward (it **inflates**), which together give strong eventual consistency;
//! - nothing is silently lost: labour and material entries are immutable and can only be **voided**
//!   (as in the ledger), and concurrent edits to a field are kept as siblings and surfaced;
//! - conflicts that need a person (an entry recorded without knowledge that the order was closed,
//!   concurrent edits of one field) are **computed from the state**, so every replica reports exactly
//!   the same ones.
//!
//! The lattice laws are checked exhaustively over small finite domains and randomly over large ones
//! (`tests/`), and the protocol is model-checked in `formal/tla/SyncMerge.tla`.
//!
//! No clocks and no entropy: ordering uses Lamport counters with the replica id as tie-break, so the
//! crate is deterministic and runs unchanged under simulation (ADR 046).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Identifies a device or server holding a replica.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReplicaId(pub u16);

/// A totally ordered logical timestamp: Lamport counter first, replica id breaking ties.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Stamp {
    /// Lamport counter.
    pub counter: u64,
    /// The replica that issued it.
    pub replica: ReplicaId,
}

/// Where a work order is in its life. Order is the lifecycle order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rank {
    /// Waiting for approval.
    Wappr,
    /// Approved.
    Appr,
    /// In progress.
    Inprg,
    /// Work complete.
    Comp,
    /// Closed to further change.
    Close,
}

impl Rank {
    fn next(self) -> Option<Self> {
        match self {
            Self::Wappr => Some(Self::Appr),
            Self::Appr => Some(Self::Inprg),
            Self::Inprg => Some(Self::Comp),
            Self::Comp => Some(Self::Close),
            Self::Close => None,
        }
    }

    const ALL: [Self; 5] = [
        Self::Wappr,
        Self::Appr,
        Self::Inprg,
        Self::Comp,
        Self::Close,
    ];

    fn index(self) -> usize {
        Self::ALL.iter().position(|r| *r == self).unwrap_or(0)
    }
}

/// Lifecycle position. A reopen starts a new `epoch` at `Inprg`, so reopening is also an upward move.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Status {
    /// Number of reopenings so far.
    pub epoch: u32,
    /// Position within the epoch.
    pub rank: Rank,
}

impl Status {
    /// A new work order.
    pub const START: Self = Self {
        epoch: 0,
        rank: Rank::Wappr,
    };
}

/// What an entry records.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EntryKind {
    /// Labour time.
    Labor,
    /// Material used.
    Material,
}

/// Identifies an entry: the replica that created it and that replica's running number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntryId {
    /// Creating replica.
    pub replica: ReplicaId,
    /// Per-replica sequence number.
    pub seq: u64,
}

/// An immutable record of labour or material. Corrections are made by voiding and adding a new entry.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Entry {
    /// Identity.
    pub id: EntryId,
    /// Labour or material.
    pub kind: EntryKind,
    /// Quantity in minor units (minutes, pieces).
    pub amount: u64,
    /// Free text.
    pub note: String,
    /// The work order status the author saw when creating it. This is what makes late entries visible.
    pub observed: Status,
}

/// One write to a field, with the write it overwrote (`base`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Write {
    /// When it was written.
    pub stamp: Stamp,
    /// The stamp of the value the writer saw, if any.
    pub base: Option<Stamp>,
    /// The new value.
    pub value: String,
}

/// A problem a person should look at. Computed from state, so identical on every replica.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Conflict {
    /// An entry was recorded by someone who did not know the work order had already been completed.
    LateEntry {
        /// The entry in question.
        entry: EntryId,
    },
    /// The newest value of a field is one of several concurrent writes over the same previous value.
    FieldSiblings {
        /// The field.
        field: String,
        /// Stamps of the concurrent writes, ascending.
        stamps: Vec<Stamp>,
    },
}

/// The replicated work order. Everything in it only grows.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Doc {
    status: Option<Status>,
    fields: BTreeMap<String, BTreeSet<Write>>,
    entries: BTreeMap<EntryId, Entry>,
    voids: BTreeSet<EntryId>,
}

impl Doc {
    /// An empty work order at the start of its life.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Current lifecycle position.
    #[must_use]
    pub fn status(&self) -> Status {
        self.status.unwrap_or(Status::START)
    }

    /// The winning value of a field: the write with the greatest stamp.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&str> {
        self.fields
            .get(name)
            .and_then(|w| w.iter().next_back())
            .map(|w| w.value.as_str())
    }

    /// Every write ever made to a field, oldest first (nothing is discarded).
    #[must_use]
    pub fn history(&self, name: &str) -> Vec<&Write> {
        self.fields
            .get(name)
            .map(|w| w.iter().collect())
            .unwrap_or_default()
    }

    /// Entries that have not been voided, in id order.
    #[must_use]
    pub fn live_entries(&self) -> Vec<&Entry> {
        self.entries
            .values()
            .filter(|e| !self.voids.contains(&e.id))
            .collect()
    }

    /// Every entry ever created, voided or not.
    #[must_use]
    pub fn all_entries(&self) -> Vec<&Entry> {
        self.entries.values().collect()
    }

    /// Whether an entry has been voided.
    #[must_use]
    pub fn is_void(&self, id: EntryId) -> bool {
        self.voids.contains(&id)
    }

    /// Sum of live entries of one kind. Wide integers: this cannot overflow.
    #[must_use]
    pub fn total(&self, kind: EntryKind) -> u128 {
        self.live_entries()
            .iter()
            .filter(|e| e.kind == kind)
            .map(|e| u128::from(e.amount))
            .sum()
    }

    /// The largest Lamport counter in the document (a replica must never issue a smaller one).
    #[must_use]
    pub fn max_counter(&self) -> u64 {
        self.fields
            .values()
            .flatten()
            .map(|w| w.stamp.counter)
            .max()
            .unwrap_or(0)
    }

    /// The least upper bound of two documents. Commutative, associative and idempotent.
    pub fn merge(&mut self, other: &Self) {
        self.status = self.status.max(other.status);
        for (name, writes) in &other.fields {
            self.fields
                .entry(name.clone())
                .or_default()
                .extend(writes.iter().cloned());
        }
        for (id, theirs) in &other.entries {
            match self.entries.get(id) {
                Some(mine) if mine >= theirs => {}
                _ => {
                    self.entries.insert(*id, theirs.clone());
                }
            }
        }
        self.voids.extend(other.voids.iter().copied());
    }

    /// Whether `self` is at or below `other` in the lattice (everything in `self` is in `other`).
    #[must_use]
    pub fn le(&self, other: &Self) -> bool {
        let mut joined = other.clone();
        joined.merge(self);
        joined == *other
    }

    /// Problems for a person to review, sorted. Same state, same answer, on every replica.
    #[must_use]
    pub fn conflicts(&self) -> Vec<Conflict> {
        let mut out = Vec::new();
        for e in self.live_entries() {
            let completed = Status {
                epoch: e.observed.epoch,
                rank: Rank::Comp,
            };
            if e.observed < completed && self.status() >= completed {
                out.push(Conflict::LateEntry { entry: e.id });
            }
        }
        for (name, writes) in &self.fields {
            let Some(top) = writes.iter().next_back() else {
                continue;
            };
            let siblings: Vec<Stamp> = writes
                .iter()
                .filter(|w| w.base == top.base)
                .map(|w| w.stamp)
                .collect();
            if siblings.len() >= 2 {
                out.push(Conflict::FieldSiblings {
                    field: name.clone(),
                    stamps: siblings,
                });
            }
        }
        out.sort();
        out
    }
}

/// Why a local operation was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncError {
    /// The work order is already closed.
    AlreadyClosed,
    /// Only a completed or closed work order can be reopened.
    NotCompleted,
    /// The entry does not exist on this replica.
    UnknownEntry(EntryId),
}

impl fmt::Display for SyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for SyncError {}

/// A replica: a document plus the counters it needs to issue unique stamps and ids.
#[derive(Clone, Debug)]
pub struct Replica {
    id: ReplicaId,
    counter: u64,
    next_seq: u64,
    doc: Doc,
}

impl Replica {
    /// A fresh replica with an empty document.
    #[must_use]
    pub fn new(id: ReplicaId) -> Self {
        Self {
            id,
            counter: 0,
            next_seq: 0,
            doc: Doc::new(),
        }
    }

    /// This replica's id.
    #[must_use]
    pub fn id(&self) -> ReplicaId {
        self.id
    }

    /// The current document.
    #[must_use]
    pub fn doc(&self) -> &Doc {
        &self.doc
    }

    fn tick(&mut self) -> Stamp {
        self.counter += 1;
        Stamp {
            counter: self.counter,
            replica: self.id,
        }
    }

    /// Writes a field, remembering which value it overwrote.
    pub fn set_field(&mut self, name: &str, value: &str) {
        let base = self
            .doc
            .fields
            .get(name)
            .and_then(|w| w.iter().next_back())
            .map(|w| w.stamp);
        let stamp = self.tick();
        self.doc
            .fields
            .entry(name.to_owned())
            .or_default()
            .insert(Write {
                stamp,
                base,
                value: value.to_owned(),
            });
    }

    /// Moves to the next lifecycle position.
    ///
    /// # Errors
    ///
    /// [`SyncError::AlreadyClosed`] when there is no next position.
    pub fn advance(&mut self) -> Result<Status, SyncError> {
        let cur = self.doc.status();
        let rank = cur.rank.next().ok_or(SyncError::AlreadyClosed)?;
        let next = Status {
            epoch: cur.epoch,
            rank,
        };
        self.doc.status = Some(next);
        Ok(next)
    }

    /// Reopens a completed or closed work order into a new epoch.
    ///
    /// # Errors
    ///
    /// [`SyncError::NotCompleted`] unless the work order is completed or closed.
    pub fn reopen(&mut self) -> Result<Status, SyncError> {
        let cur = self.doc.status();
        if cur.rank < Rank::Comp {
            return Err(SyncError::NotCompleted);
        }
        let next = Status {
            epoch: cur.epoch + 1,
            rank: Rank::Inprg,
        };
        self.doc.status = Some(next);
        Ok(next)
    }

    /// Records labour or material, noting the status the author currently sees.
    pub fn add_entry(&mut self, kind: EntryKind, amount: u64, note: &str) -> EntryId {
        let id = EntryId {
            replica: self.id,
            seq: self.next_seq,
        };
        self.next_seq += 1;
        let entry = Entry {
            id,
            kind,
            amount,
            note: note.to_owned(),
            observed: self.doc.status(),
        };
        self.doc.entries.insert(id, entry);
        id
    }

    /// Voids an entry (entries are never edited or deleted).
    ///
    /// # Errors
    ///
    /// [`SyncError::UnknownEntry`] when this replica has not seen the entry.
    pub fn void_entry(&mut self, id: EntryId) -> Result<(), SyncError> {
        if !self.doc.entries.contains_key(&id) {
            return Err(SyncError::UnknownEntry(id));
        }
        self.doc.voids.insert(id);
        Ok(())
    }

    /// Receives another replica's document (or a relayed copy; duplicates and reordering are harmless).
    pub fn receive(&mut self, other: &Doc) {
        self.doc.merge(other);
        self.counter = self.counter.max(self.doc.max_counter());
    }
}

/// Canonical text form of a document: sorted, deterministic, and round-trippable. Hash or sign this.
pub mod wire {
    use super::{Doc, Entry, EntryId, EntryKind, Rank, ReplicaId, Stamp, Status, Write};
    use std::fmt::Write as _;

    /// A malformed encoded document.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct DecodeError(pub String);

    fn esc(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                '%' => out.push_str("%25"),
                ' ' => out.push_str("%20"),
                '\n' => out.push_str("%0A"),
                '\r' => out.push_str("%0D"),
                '\t' => out.push_str("%09"),
                _ => out.push(c),
            }
        }
        out
    }

    fn unesc(s: &str) -> Result<String, DecodeError> {
        let mut out = String::new();
        let mut it = s.chars();
        while let Some(c) = it.next() {
            if c != '%' {
                out.push(c);
                continue;
            }
            let hex: String = it.by_ref().take(2).collect();
            let byte = u8::from_str_radix(&hex, 16)
                .map_err(|_| DecodeError(format!("bad escape in {s:?}")))?;
            out.push(char::from(byte));
        }
        Ok(out)
    }

    fn stamp(s: Stamp) -> String {
        format!("{}.{}", s.counter, s.replica.0)
    }

    fn opt_stamp(s: Option<Stamp>) -> String {
        s.map_or_else(|| "-".to_owned(), stamp)
    }

    /// Encodes a document.
    #[must_use]
    pub fn encode(doc: &Doc) -> String {
        let mut out = String::new();
        let st = doc.status();
        let _ = writeln!(out, "status {} {}", st.epoch, st.rank.index());
        for (name, writes) in &doc.fields {
            for w in writes {
                let _ = writeln!(
                    out,
                    "field {} {} {} {}",
                    esc(name),
                    stamp(w.stamp),
                    opt_stamp(w.base),
                    esc(&w.value)
                );
            }
        }
        for e in doc.entries.values() {
            let kind = if e.kind == EntryKind::Labor { 'L' } else { 'M' };
            let _ = writeln!(
                out,
                "entry {}.{} {kind} {} {}.{} {}",
                e.id.replica.0,
                e.id.seq,
                e.amount,
                e.observed.epoch,
                e.observed.rank.index(),
                esc(&e.note)
            );
        }
        for v in &doc.voids {
            let _ = writeln!(out, "void {}.{}", v.replica.0, v.seq);
        }
        out
    }

    fn pair<A: std::str::FromStr, B: std::str::FromStr>(s: &str) -> Result<(A, B), DecodeError> {
        let bad = || DecodeError(format!("bad pair {s:?}"));
        let (a, b) = s.split_once('.').ok_or_else(bad)?;
        Ok((a.parse().map_err(|_| bad())?, b.parse().map_err(|_| bad())?))
    }

    fn parse_stamp(s: &str) -> Result<Stamp, DecodeError> {
        let (counter, replica): (u64, u16) = pair(s)?;
        Ok(Stamp {
            counter,
            replica: ReplicaId(replica),
        })
    }

    fn rank(i: &str) -> Result<Rank, DecodeError> {
        i.parse::<usize>()
            .ok()
            .and_then(|n| Rank::ALL.get(n).copied())
            .ok_or_else(|| DecodeError(format!("bad rank {i:?}")))
    }

    /// Decodes a document produced by [`encode`].
    ///
    /// # Errors
    ///
    /// [`DecodeError`] naming the offending line.
    pub fn decode(text: &str) -> Result<Doc, DecodeError> {
        let mut doc = Doc::new();
        for line in text.lines().filter(|l| !l.is_empty()) {
            let f: Vec<&str> = line.split(' ').collect();
            let bad = || DecodeError(format!("bad line {line:?}"));
            match f.as_slice() {
                ["status", epoch, r] => {
                    doc.status = Some(Status {
                        epoch: epoch.parse().map_err(|_| bad())?,
                        rank: rank(r)?,
                    });
                }
                ["field", name, st, base, value] => {
                    let base = if *base == "-" {
                        None
                    } else {
                        Some(parse_stamp(base)?)
                    };
                    doc.fields.entry(unesc(name)?).or_default().insert(Write {
                        stamp: parse_stamp(st)?,
                        base,
                        value: unesc(value)?,
                    });
                }
                ["entry", id, kind, amount, obs, note] => {
                    let (replica, seq): (u16, u64) = pair(id)?;
                    let (epoch, r): (u32, String) = pair(obs)?;
                    let entry = Entry {
                        id: EntryId {
                            replica: ReplicaId(replica),
                            seq,
                        },
                        kind: match *kind {
                            "L" => EntryKind::Labor,
                            "M" => EntryKind::Material,
                            _ => return Err(bad()),
                        },
                        amount: amount.parse().map_err(|_| bad())?,
                        note: unesc(note)?,
                        observed: Status {
                            epoch,
                            rank: rank(&r)?,
                        },
                    };
                    doc.entries.insert(entry.id, entry);
                }
                ["void", id] => {
                    let (replica, seq): (u16, u64) = pair(id)?;
                    doc.voids.insert(EntryId {
                        replica: ReplicaId(replica),
                        seq,
                    });
                }
                _ => return Err(bad()),
            }
        }
        Ok(doc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(n: u16) -> Replica {
        Replica::new(ReplicaId(n))
    }

    #[test]
    fn lifecycle_is_ordered_and_reopen_starts_a_new_epoch() {
        let mut a = r(1);
        assert_eq!(a.advance().unwrap().rank, Rank::Appr);
        a.advance().unwrap();
        a.advance().unwrap();
        a.advance().unwrap();
        assert_eq!(
            a.doc().status(),
            Status {
                epoch: 0,
                rank: Rank::Close
            }
        );
        assert_eq!(a.advance(), Err(SyncError::AlreadyClosed));
        assert_eq!(
            a.reopen().unwrap(),
            Status {
                epoch: 1,
                rank: Rank::Inprg
            }
        );
        assert_eq!(r(2).reopen(), Err(SyncError::NotCompleted));
    }

    #[test]
    fn concurrent_progress_and_closure_merge_to_the_furthest_state() {
        let (mut desk, mut phone) = (r(1), r(2));
        phone.advance().unwrap();
        for _ in 0..4 {
            desk.advance().unwrap();
        }
        let (d, p) = (desk.doc().clone(), phone.doc().clone());
        desk.receive(&p);
        phone.receive(&d);
        assert_eq!(desk.doc(), phone.doc());
        assert_eq!(desk.doc().status().rank, Rank::Close);
    }

    #[test]
    fn an_entry_made_without_knowing_of_the_closure_is_kept_and_flagged_everywhere() {
        let (mut desk, mut phone) = (r(1), r(2));
        for _ in 0..2 {
            desk.advance().unwrap();
            phone.receive(desk.doc());
        }
        assert_eq!(phone.doc().status().rank, Rank::Inprg);
        // The phone goes offline in progress; the desk completes and closes.
        let snapshot_before = phone.doc().clone();
        desk.advance().unwrap();
        desk.advance().unwrap();
        let late = phone.add_entry(EntryKind::Labor, 90, "replaced bearing");
        desk.receive(phone.doc());
        phone.receive(desk.doc());
        assert_eq!(desk.doc(), phone.doc());
        assert_eq!(desk.doc().live_entries().len(), 1, "the entry is not lost");
        assert_eq!(
            desk.doc().conflicts(),
            vec![Conflict::LateEntry { entry: late }]
        );
        assert_eq!(
            phone.doc().conflicts(),
            desk.doc().conflicts(),
            "every replica reports the same conflicts"
        );
        assert!(snapshot_before.le(desk.doc()));
    }

    #[test]
    fn an_entry_made_knowing_the_work_was_complete_is_not_a_conflict() {
        let (mut desk, mut phone) = (r(1), r(2));
        for _ in 0..3 {
            desk.advance().unwrap();
        }
        phone.receive(desk.doc());
        phone.add_entry(EntryKind::Material, 2, "gasket");
        desk.receive(phone.doc());
        assert!(desk.doc().conflicts().is_empty());
    }

    #[test]
    fn concurrent_field_edits_are_kept_as_siblings_until_someone_resolves_them() {
        let (mut a, mut b) = (r(1), r(2));
        a.set_field("description", "pump noisy");
        b.receive(a.doc());
        a.set_field("description", "pump noisy, bearing");
        b.set_field("description", "pump leaking");
        let (da, db) = (a.doc().clone(), b.doc().clone());
        a.receive(&db);
        b.receive(&da);
        assert_eq!(a.doc(), b.doc());
        assert_eq!(
            a.doc().history("description").len(),
            3,
            "no write is discarded"
        );
        assert!(matches!(
            a.doc().conflicts().as_slice(),
            [Conflict::FieldSiblings { .. }]
        ));
        // Resolving means writing after seeing both.
        a.set_field("description", "pump leaking at bearing");
        assert!(a.doc().conflicts().is_empty());
        b.receive(a.doc());
        assert_eq!(
            b.doc().field("description"),
            Some("pump leaking at bearing")
        );
    }

    #[test]
    fn voiding_is_permanent_and_sums_exclude_voided_entries() {
        let (mut a, mut b) = (r(1), r(2));
        let e1 = a.add_entry(EntryKind::Labor, 60, "inspect");
        let _e2 = a.add_entry(EntryKind::Labor, 30, "repair");
        b.receive(a.doc());
        b.void_entry(e1).unwrap();
        a.receive(b.doc());
        a.receive(b.doc());
        assert_eq!(a.doc().total(EntryKind::Labor), 30);
        assert!(a.doc().is_void(e1));
        assert_eq!(
            a.void_entry(EntryId {
                replica: ReplicaId(9),
                seq: 9
            }),
            Err(SyncError::UnknownEntry(EntryId {
                replica: ReplicaId(9),
                seq: 9
            }))
        );
    }

    #[test]
    fn lamport_counters_never_go_backwards_after_a_merge() {
        let (mut a, mut b) = (r(1), r(2));
        for i in 0..5 {
            a.set_field("x", &i.to_string());
        }
        b.receive(a.doc());
        b.set_field("x", "later");
        assert_eq!(
            b.doc().field("x"),
            Some("later"),
            "a write after receiving wins over what was received"
        );
    }

    #[test]
    fn wire_format_round_trips_awkward_text() {
        let mut a = r(3);
        a.set_field("note with spaces", "line1\nline2\t100% sure \u{e9}");
        a.add_entry(EntryKind::Material, 7, "%20 literally, and\nnewline");
        let id = a.add_entry(EntryKind::Labor, 1, "");
        a.void_entry(id).unwrap();
        a.advance().unwrap();
        let text = wire::encode(a.doc());
        assert_eq!(wire::decode(&text).unwrap(), *a.doc());
        assert_eq!(
            wire::encode(&wire::decode(&text).unwrap()),
            text,
            "encoding is canonical"
        );
    }

    #[test]
    fn decoding_garbage_is_an_error_not_a_panic() {
        for bad in [
            "nonsense",
            "status x y",
            "status 0 9",
            "field a 1.1",
            "entry 1.1 Z 5 0.0 n",
            "void 1",
            "status 0 0\nvoid a.b",
        ] {
            assert!(wire::decode(bad).is_err(), "{bad:?}");
        }
    }
}
