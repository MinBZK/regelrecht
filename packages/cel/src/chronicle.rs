//! The chronicle: append-only storage of grams, one JSON line per gram in
//! `DATA_DIR/<cell>/<chronicle>.jsonl`.
//!
//! There is deliberately no path to change or delete a gram. A correction or
//! a remedy is a new gram.
//!
//! The file is the source; the grams are also held in memory, with an index on
//! id and an index per root, so a reduction or a query for a group does not
//! reread the file on every request. The root of a gram is the root of the
//! gram it refers to; a gram without a reference is its own root (see
//! [`crate::gram`]). The memory only grows, under the same lock as the
//! writing: what is in memory is on disk. Whoever runs the runtime therefore
//! does not write to the file themselves.
//!
//! A line only counts once it ends with a newline. An incomplete last line
//! (the runtime stopped in the middle of writing) is truncated on opening and
//! reported; such a gram was never confirmed either. An unreadable line before
//! it is not a write error but a broken chronicle, and then the chronicle does
//! not open.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex, MutexGuard, OnceLock, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard,
};

use chrono::{DateTime, FixedOffset};

use crate::gram::Gram;

/// A recorded gram, with its YAML once someone asked for it: the gram no
/// longer changes, so the YAML only has to be made once.
#[derive(Debug)]
pub struct Recorded {
    pub gram: Gram,
    yaml: OnceLock<String>,
}

impl Recorded {
    fn new(gram: Gram) -> Arc<Self> {
        Arc::new(Self {
            gram,
            yaml: OnceLock::new(),
        })
    }

    /// The YAML of the gram, made with `make` if it is not there yet.
    pub fn yaml(
        &self,
        make: impl FnOnce(&Gram) -> Result<String, String>,
    ) -> Result<String, String> {
        if let Some(y) = self.yaml.get() {
            return Ok(y.clone());
        }
        let y = make(&self.gram)?;
        Ok(self.yaml.get_or_init(|| y).clone())
    }
}

/// The grams of a chronicle in memory.
#[derive(Default)]
struct Stack {
    grams: Vec<Arc<Recorded>>,
    /// The length of the file: up to here there is a whole line.
    length: u64,
}

/// All loaded chronicles, with the indexes across the chronicles: a gram may
/// refer to a gram in another chronicle of the same cell.
#[derive(Default)]
struct ChronicleState {
    stacks: BTreeMap<String, Stack>,
    /// id -> the gram.
    per_id: HashMap<String, Arc<Recorded>>,
    /// root -> the grams with that root, in recording order.
    per_root: HashMap<String, Vec<Arc<Recorded>>>,
}

impl ChronicleState {
    /// Add a gram whose root is known.
    fn add(&mut self, gram: Gram) -> Arc<Recorded> {
        let root = gram.root.clone().unwrap_or_else(|| gram.id.clone());
        let v = Recorded::new(gram);
        self.per_id.insert(v.gram.id.clone(), v.clone());
        self.per_root.entry(root).or_default().push(v.clone());
        self.stacks
            .entry(v.gram.chronicle.clone())
            .or_default()
            .grams
            .push(v.clone());
        v
    }

    /// The root of a gram that refers to targets: that of the targets, if
    /// they are all there and have the same root. Without a reference, its
    /// own id.
    fn root_of(&self, gram: &Gram, extra: &HashMap<String, String>) -> Root {
        if gram.refers_to.is_empty() {
            return Root::Own;
        }
        let mut found: Option<String> = None;
        for id in gram.refers_to.values() {
            let w = match self.per_id.get(id) {
                Some(v) => v.gram.root.clone().unwrap_or_else(|| v.gram.id.clone()),
                None => match extra.get(id) {
                    Some(w) => w.clone(),
                    None => return Root::Unknown(id.clone()),
                },
            };
            match &found {
                Some(g) if *g != w => return Root::Different,
                _ => found = Some(w),
            }
        }
        found.map_or(Root::Own, Root::Of)
    }

    /// The grams with this root in these chronicles, in recording order.
    fn of_the_root(&self, chronicles: &[&str], root: &str) -> Vec<Arc<Recorded>> {
        self.per_root
            .get(root)
            .into_iter()
            .flatten()
            .filter(|v| chronicles.contains(&v.gram.chronicle.as_str()))
            .cloned()
            .collect()
    }
}

/// What the root of a gram is.
enum Root {
    /// The gram refers to nothing: it is its own root.
    Own,
    /// The root of the grams it refers to.
    Of(String),
    /// The gram refers to an id that is not loaded (yet).
    Unknown(String),
    /// The grams it refers to have different roots.
    Different,
}

/// What a check sees, owned (see [`Chronicle::view_for`]): the grams a gram
/// refers to, the group of its root and that root.
pub struct ViewOwn {
    targets: Vec<Arc<Recorded>>,
    group: Vec<Arc<Recorded>>,
    pub root: Option<String>,
    root_error: Option<String>,
}

impl ViewOwn {
    /// The view to perform a check with.
    pub fn view(&self) -> View<'_> {
        View {
            targets: self
                .targets
                .iter()
                .map(|v| (v.gram.id.clone(), &v.gram))
                .collect(),
            group: self.group.iter().map(|v| &v.gram).collect(),
            root_error: self.root_error.clone(),
        }
    }
}

impl ChronicleState {
    /// What a check on `gram` sees, in `chronicles`.
    fn view(&self, chronicles: &[&str], gram: &Gram) -> ViewOwn {
        let targets: Vec<Arc<Recorded>> = gram
            .refers_to
            .values()
            .filter_map(|id| self.per_id.get(id).cloned())
            .collect();
        let (root, root_error) = match self.root_of(gram, &HashMap::new()) {
            Root::Own => (None, None),
            Root::Of(w) => (Some(w), None),
            Root::Unknown(id) => (None, Some(format!("no gram '{id}' in the chronicle"))),
            Root::Different => (
                None,
                Some("the gram refers to grams that do not belong to the same root".into()),
            ),
        };
        let group = root
            .as_deref()
            .map(|w| self.of_the_root(chronicles, w))
            .unwrap_or_default();
        ViewOwn {
            targets,
            group,
            root,
            root_error,
        }
    }
}

/// What a check sees when recording (see [`Chronicle::record_provided`]):
/// the grams the new gram refers to, and the grams of its root, as they are
/// recorded at that moment.
pub struct View<'a> {
    /// Per id the gram refers to, that gram, if it is there.
    pub targets: BTreeMap<String, &'a Gram>,
    /// The grams with the same root as the new gram, in recording order;
    /// empty if the gram is its own root.
    pub group: Vec<&'a Gram>,
    /// Why the gram got no root, if so.
    pub root_error: Option<String>,
}

pub struct Chronicle {
    dir: PathBuf,
    /// One writer at a time, so lines do not get interleaved and a check sees
    /// what is actually there. Readers do not wait for it: the disk (fsync)
    /// happens without holding `state`, and only writers change `state`.
    writer: Mutex<()>,
    /// The grams per chronicle, and the indexes.
    state: RwLock<ChronicleState>,
}

/// A line as JSONL, with a newline.
fn as_row(gram: &Gram) -> Result<String, String> {
    gram.validate()
        .map_err(|f| format!("gram does not validate: {}", f.join("; ")))?;
    let mut row = serde_json::to_string(gram).map_err(|e| e.to_string())?;
    row.push('\n');
    Ok(row)
}

impl Chronicle {
    /// Open (and create if needed) the directory of chronicles, and read
    /// `chronicles` into memory. An incomplete last line is truncated here.
    pub fn open(map: &Path, chronicles: &[&str]) -> Result<Self, String> {
        std::fs::create_dir_all(map).map_err(|e| format!("{}: {e}", map.display()))?;
        let k = Self {
            dir: map.to_path_buf(),
            writer: Mutex::new(()),
            state: RwLock::new(ChronicleState::default()),
        };
        k.load(chronicles)?;
        Ok(k)
    }

    fn file(&self, chronicle: &str) -> Result<PathBuf, String> {
        // The schema only allows [a-z0-9_]; checked again here, because this
        // becomes a file name.
        let valid = !chronicle.is_empty()
            && chronicle
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
        if !valid {
            return Err(format!("invalid chronicle '{chronicle}'"));
        }
        Ok(self.dir.join(format!("{chronicle}.jsonl")))
    }

    // A poisoned lock (a thread panicked while holding it) stays usable:
    // `state` only changes after a successful write, in one step, so nothing
    // is ever half updated.
    fn write_lock(&self) -> MutexGuard<'_, ()> {
        self.writer.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn read_state(&self) -> RwLockReadGuard<'_, ChronicleState> {
        self.state.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn write_state(&self) -> RwLockWriteGuard<'_, ChronicleState> {
        self.state.write().unwrap_or_else(PoisonError::into_inner)
    }

    /// Make sure these chronicles are in memory; a chronicle that is not there
    /// yet is read from disk. The roots of the new grams come from their
    /// references, also to a gram in another chronicle loaded at the same
    /// time. A gram that refers to an unknown id becomes its own root, with a
    /// warning.
    fn load(&self, chronicles: &[&str]) -> Result<(), String> {
        let absent: Vec<&str> = {
            let state = self.read_state();
            chronicles
                .iter()
                .copied()
                .filter(|k| !state.stacks.contains_key(*k))
                .collect()
        };
        if absent.is_empty() {
            return Ok(());
        }
        // Under the write lock, so nobody writes to the file while it is being
        // read (and truncated if needed).
        let _writer = self.write_lock();
        let mut new: Vec<(String, u64, Vec<Gram>)> = Vec::new();
        for k in absent {
            if self.read_state().stacks.contains_key(k) || new.iter().any(|(n, _, _)| n == k) {
                continue;
            }
            let (length, grams) = read_file(&self.file(k)?, k)?;
            new.push((k.to_string(), length, grams));
        }
        let mut state = self.write_state();
        // A duplicate id refuses the whole load before anything changes: a
        // retry must not find half of it in memory.
        let mut seen: HashSet<&str> = HashSet::new();
        for (k, _, grams) in &new {
            for g in grams {
                if state.per_id.contains_key(&g.id) || !seen.insert(g.id.as_str()) {
                    return Err(format!("chronicle '{k}': id {} occurs twice", g.id));
                }
            }
        }
        // The roots, in rounds: a gram can refer to a gram that comes later in
        // the list (another chronicle).
        let mut known: HashMap<String, String> = HashMap::new();
        loop {
            let mut further = false;
            for (_, _, grams) in &mut new {
                for g in grams.iter_mut().filter(|g| g.root.is_none()) {
                    let w = match state.root_of(g, &known) {
                        Root::Own => g.id.clone(),
                        Root::Of(w) => w,
                        Root::Unknown(_) | Root::Different => continue,
                    };
                    known.insert(g.id.clone(), w.clone());
                    g.root = Some(w);
                    further = true;
                }
            }
            if !further {
                break;
            }
        }
        for (k, length, grams) in new {
            let mut loose = 0_usize;
            state.stacks.entry(k.clone()).or_default().length = length;
            for mut g in grams {
                if g.root.is_none() {
                    loose += 1;
                    g.root = Some(g.id.clone());
                }
                state.add(g);
            }
            if loose > 0 {
                tracing::warn!(chronicle = %k, grams = loose, "grams that refer to an unknown gram (or to grams of different roots): read as their own root");
            }
        }
        Ok(())
    }

    /// Read from the stacks of `chronicles`. Reading never goes to disk:
    /// [`Chronicle::open`] loads the chronicles of the cell, so a reader (often
    /// on an async thread) does not wait for a file. A chronicle that is not
    /// opened is an error.
    fn with_state<T>(
        &self,
        chronicles: &[&str],
        f: impl FnOnce(&ChronicleState) -> T,
    ) -> Result<T, String> {
        let state = self.read_state();
        for k in chronicles {
            self.file(k)?;
            if !state.stacks.contains_key(*k) {
                return Err(format!("chronicle '{k}' is not opened"));
            }
        }
        Ok(f(&state))
    }

    /// Add a gram. The gram must validate against `gram.json`.
    pub fn add(&self, gram: &Gram) -> Result<(), String> {
        self.add_provided(gram, &[], |_, _| Ok::<(), std::convert::Infallible>(()))?
            .map(|_| ())
            .map_err(|never| match never {})
    }

    /// Add a gram if `check` allows it. The check sees the grams the gram
    /// refers to and the grams of its root in `chronicles` (from the indexes,
    /// without copying the rest of the chronicle; see [`View`]), as they are
    /// recorded at that moment, under the same lock as the writing. This way
    /// two concurrent requests cannot both pass a check that should have
    /// stopped the other. The gram gets its root before the check.
    ///
    /// The outer error is a storage error; the inner one is the refusal of the
    /// check, and then nothing has been recorded.
    pub fn add_provided<E>(
        &self,
        gram: &Gram,
        chronicles: &[&str],
        check: impl FnOnce(&mut Gram, &View<'_>) -> Result<(), E>,
    ) -> Result<Result<Arc<Recorded>, E>, String> {
        self.write_provided(
            gram.clone(),
            chronicles,
            None::<(fn() -> _, fn(String) -> E)>,
            check,
        )
    }

    /// Record a gram like [`Chronicle::add_provided`], and first set, under the
    /// write lock, the moment of recording from `clock` (see [`Gram::stamp`])
    /// and a new id (a uuid v7 at that moment). This way the order of the
    /// lines in the file is that of their `recorded_at`, even when two
    /// requests come in at the same time. The check sees the stamped gram.
    /// Answer: the gram as recorded. If the stamp refuses the gram (a bound
    /// `effective_at` after the recording), `refuse` turns that into the
    /// refusal. The check may complete the gram with what is only fixed under
    /// the lock.
    pub fn record_provided<E>(
        &self,
        gram: Gram,
        chronicles: &[&str],
        clock: impl FnOnce() -> DateTime<FixedOffset>,
        refuse: impl FnOnce(String) -> E,
        check: impl FnOnce(&mut Gram, &View<'_>) -> Result<(), E>,
    ) -> Result<Result<Arc<Recorded>, E>, String> {
        self.write_provided(gram, chronicles, Some((clock, refuse)), check)
    }

    fn write_provided<E>(
        &self,
        mut gram: Gram,
        chronicles: &[&str],
        clock: Option<(
            impl FnOnce() -> DateTime<FixedOffset>,
            impl FnOnce(String) -> E,
        )>,
        check: impl FnOnce(&mut Gram, &View<'_>) -> Result<(), E>,
    ) -> Result<Result<Arc<Recorded>, E>, String> {
        let path = self.file(&gram.chronicle)?;
        let mut all = chronicles.to_vec();
        all.push(gram.chronicle.as_str());
        self.load(&all)?;
        let _writer = self.write_lock();
        // As long as we hold the write lock, `state` does not change: the
        // check sees what is there, even after the read lock is released.
        let (view, length, last) = {
            let state = self.read_state();
            let view = state.view(chronicles, &gram);
            gram.root = view.root.clone();
            let stack = state.stacks.get(&gram.chronicle);
            let length = stack.map_or(0, |s| s.length);
            let last = stack.and_then(|s| s.grams.last()).cloned();
            (view, length, last)
        };
        if let Some((clock, refuse)) = clock {
            let not_for = last.map(|v| v.gram.recorded()).transpose()?;
            let now = clock();
            if let Err(f) = gram.stamp(now, not_for) {
                return Ok(Err(refuse(f)));
            }
            gram.id = crate::gram::new_id(gram.recorded()?);
        }
        if gram.root.is_none() && view.root_error.is_none() {
            gram.root = Some(gram.id.clone());
        }
        let view = view.view();
        if let Err(w) = check(&mut gram, &view) {
            return Ok(Err(w));
        }
        if let Some(f) = view.root_error {
            return Err(f);
        }
        if self.read_state().per_id.contains_key(&gram.id) {
            return Err(format!("a gram with id {} is already there", gram.id));
        }
        let row = as_row(&gram)?;
        write_row(&path, length, row.as_bytes())?;
        let mut state = self.write_state();
        state
            .stacks
            .entry(gram.chronicle.clone())
            .or_default()
            .length = length + row.len() as u64;
        Ok(Ok(state.add(gram)))
    }

    /// Put the initial state into the chronicle, if every chronicle of
    /// `chronicles` is empty; false if something was already there. Each file
    /// is written in one go (a temporary file, then renamed), so an
    /// interrupted start leaves no half file behind. If the initial state
    /// spans more than one chronicle, that holds per file, not across files.
    pub fn set_initial_state(&self, chronicles: &[&str], grams: &[Gram]) -> Result<bool, String> {
        let mut per_chronicle: BTreeMap<&str, String> = BTreeMap::new();
        for g in grams {
            per_chronicle
                .entry(g.chronicle.as_str())
                .or_default()
                .push_str(&as_row(g)?);
        }
        let mut all: Vec<&str> = chronicles.to_vec();
        all.extend(per_chronicle.keys());
        self.load(&all)?;
        let _writer = self.write_lock();
        {
            let state = self.read_state();
            if all
                .iter()
                .any(|k| state.stacks.get(*k).is_some_and(|s| !s.grams.is_empty()))
            {
                return Ok(false);
            }
        }
        // The roots of the initial state, from its own references, in the
        // order of the initial state.
        let mut with_root: Vec<Gram> = Vec::with_capacity(grams.len());
        let mut known: HashMap<String, String> = HashMap::new();
        for g in grams {
            let mut g = g.clone();
            let mut w: Option<String> = None;
            for id in g.refers_to.values() {
                let of = known.get(id).ok_or_else(|| {
                    format!(
                        "initial_state: gram {} refers to {id}, which is not in it (earlier)",
                        g.id
                    )
                })?;
                match &w {
                    Some(other) if other != of => {
                        return Err(format!(
                        "initial_state: gram {} refers to grams of different roots ({other}, {of})",
                        g.id
                    ))
                    }
                    _ => w = Some(of.clone()),
                }
            }
            let w = w.unwrap_or_else(|| g.id.clone());
            if known.insert(g.id.clone(), w.clone()).is_some() {
                return Err(format!("initial_state: id {} occurs twice", g.id));
            }
            g.root = Some(w);
            with_root.push(g);
        }
        // First everything next to the chronicle, then rename: a rename is
        // atomic per file.
        let mut ready = Vec::new();
        for (k, text) in &per_chronicle {
            let path = self.file(k)?;
            let temporary = path.with_extension("jsonl.nieuw");
            write_file(&temporary, text.as_bytes())?;
            ready.push((temporary, path));
        }
        let mut state = self.write_state();
        for ((temporary, path), (k, text)) in ready.iter().zip(&per_chronicle) {
            std::fs::rename(temporary, path).map_err(|e| format!("{}: {e}", path.display()))?;
            // What has been renamed is also in memory, even if a later rename
            // fails.
            state.stacks.entry((*k).to_string()).or_default().length = text.len() as u64;
            for g in with_root.iter().filter(|g| g.chronicle == *k) {
                state.add(g.clone());
            }
        }
        if let Ok(d) = std::fs::File::open(&self.dir) {
            // Make the rename itself durable; if that fails, the file is there
            // anyway.
            let _ = d.sync_all();
        }
        Ok(true)
    }

    /// All grams of a chronicle, in recording order.
    pub fn read(&self, chronicle: &str) -> Result<Vec<Arc<Recorded>>, String> {
        self.all(&[chronicle])
    }

    /// All grams of these chronicles, per chronicle in recording order.
    pub fn all(&self, chronicles: &[&str]) -> Result<Vec<Arc<Recorded>>, String> {
        self.with_state(chronicles, |state| {
            chronicles
                .iter()
                .filter_map(|k| state.stacks.get(*k))
                .flat_map(|s| s.grams.iter().cloned())
                .collect()
        })
    }

    /// The grams with this root, across the given chronicles, in recording
    /// order.
    pub fn read_root(&self, chronicles: &[&str], root: &str) -> Result<Vec<Arc<Recorded>>, String> {
        self.with_state(chronicles, |state| state.of_the_root(chronicles, root))
    }

    /// What a check on `gram` would see if it were recorded now (see
    /// [`View`]), without a lock: for a trial, which records nothing. The gram
    /// gets its root (its own id if it refers to nothing).
    pub fn view_for(&self, chronicles: &[&str], gram: &mut Gram) -> Result<ViewOwn, String> {
        let z = self.with_state(chronicles, |state| state.view(chronicles, gram))?;
        gram.root = z
            .root
            .clone()
            .or_else(|| z.root_error.is_none().then(|| gram.id.clone()));
        Ok(z)
    }

    /// The gram with this id, if it is in one of these chronicles.
    pub fn gram(&self, chronicles: &[&str], id: &str) -> Result<Option<Arc<Recorded>>, String> {
        self.with_state(chronicles, |state| {
            state
                .per_id
                .get(id)
                .filter(|v| chronicles.contains(&v.gram.chronicle.as_str()))
                .cloned()
        })
    }
}

/// Read a chronicle from disk: the length up to the last whole line and the
/// grams, still without a root. A last line without a newline was written
/// incompletely: it is truncated, with a warning, but only once the rest is
/// readable. A temporary file from an interrupted initial state is removed.
/// A chronicle from before chronolex v0.2.0 (a gram without id or
/// `recorded_at`) is not converted but refused.
fn read_file(path: &Path, chronicle: &str) -> Result<(u64, Vec<Gram>), String> {
    let error = |e: std::io::Error| format!("{}: {e}", path.display());
    let temporary = path.with_extension("jsonl.nieuw");
    if temporary.exists() {
        tracing::warn!(file = %temporary.display(), "removed leftovers of an interrupted initial state");
        std::fs::remove_file(&temporary).map_err(error)?;
    }
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((0, Vec::new())),
        Err(e) => return Err(error(e)),
    };
    let whole = bytes.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
    let text = std::str::from_utf8(&bytes[..whole])
        .map_err(|e| format!("{}: not UTF-8: {e}", path.display()))?;
    let mut grams = Vec::new();
    for (i, row) in text.lines().enumerate() {
        if row.trim().is_empty() {
            continue;
        }
        let doc: serde_json::Value = serde_json::from_str(row)
            .map_err(|e| format!("{} line {}: {e}", path.display(), i + 1))?;
        if ["id", "recorded_at"].iter().any(|k| doc.get(k).is_none()) {
            return Err(format!(
                "{} line {}: a gram from before chronolex v0.2.0 (without id or recorded_at); old chronicles are not converted: start with an empty DATA_DIR for chronicle '{chronicle}'",
                path.display(),
                i + 1
            ));
        }
        let gram: Gram = serde_json::from_value(doc)
            .map_err(|e| format!("{} line {}: {e}", path.display(), i + 1))?;
        grams.push(gram);
    }
    if whole < bytes.len() {
        tracing::warn!(
            chronicle = %path.display(),
            bytes = bytes.len() - whole,
            "truncated incomplete last line: the runtime stopped while writing, and that gram was never confirmed"
        );
        let f = OpenOptions::new().write(true).open(path).map_err(error)?;
        f.set_len(whole as u64)
            .and_then(|()| f.sync_all())
            .map_err(error)?;
    }
    Ok((whole as u64, grams))
}

/// Write a line after the first `length` bytes of a chronicle, and wait until
/// it is on disk. Whatever was in the file after that (the rest of an earlier
/// failed write) is removed first, so a line never ends up after a half
/// line.
fn write_row(path: &Path, length: u64, row: &[u8]) -> Result<(), String> {
    let error = |e: std::io::Error| format!("{}: {e}", path.display());
    let mut f = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .map_err(error)?;
    f.set_len(length).map_err(error)?;
    f.seek(SeekFrom::Start(length)).map_err(error)?;
    f.write_all(row).and_then(|()| f.sync_data()).map_err(error)
}

/// Write a whole file and wait until it is on disk.
fn write_file(path: &Path, content: &[u8]) -> Result<(), String> {
    let error = |e: std::io::Error| format!("{}: {e}", path.display());
    let mut f = std::fs::File::create(path).map_err(error)?;
    f.write_all(content)
        .and_then(|()| f.sync_all())
        .map_err(error)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::gram::test_follower;

    /// A gram with a new id that refers to the gram with id `root`, or, if
    /// `root` is not in the chronicle yet, that gram itself.
    fn gram(root: &str) -> Gram {
        crate::gram::test_gram(&uuid::Uuid::now_v7().to_string()).with_reference(root)
    }

    trait WithReference {
        fn with_reference(self, target: &str) -> Gram;
    }
    impl WithReference for Gram {
        fn with_reference(mut self, target: &str) -> Gram {
            self.refers_to.insert("application".into(), target.into());
            self.root = None;
            self
        }
    }

    /// The gram that is the root `id`.
    fn root(id: &str) -> Gram {
        crate::gram::test_gram(id)
    }

    const Z1: &str = "00000000-0000-4000-8000-000000000001";
    const Z2: &str = "00000000-0000-4000-8000-000000000002";
    const K: &[&str] = &["test_kroniek"];

    fn open(dir: &Path) -> Chronicle {
        Chronicle::open(dir, K).unwrap()
    }

    fn count(k: &Chronicle) -> usize {
        k.read("test_kroniek").unwrap().len()
    }

    #[test]
    fn add_and_read() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        assert_eq!(count(&k), 0);
        k.add(&root(Z1)).unwrap();
        k.add(&root(Z2)).unwrap();
        let follower = k
            .add_provided(&gram(Z1), K, |_, _| Ok::<(), ()>(()))
            .unwrap()
            .unwrap();
        assert_eq!(follower.gram.root.as_deref(), Some(Z1));
        // A gram that refers to the follower belongs to the same root.
        let further = test_follower("decision", &follower.gram);
        k.add(&further).unwrap();
        assert_eq!(count(&k), 4);
        assert_eq!(k.read_root(K, Z1).unwrap().len(), 3);
        assert_eq!(k.gram(K, &further.id).unwrap().unwrap().gram.id, further.id);
        // After reopening the same is there, from the file, with the roots.
        drop(k);
        let k = open(dir.path());
        assert_eq!(count(&k), 4);
        assert_eq!(k.read_root(K, Z1).unwrap().len(), 3);
        assert_eq!(k.read_root(K, Z2).unwrap().len(), 1);
    }

    /// A chronicle from before v0.2.0 (without id, with a case identifier) is
    /// not converted: the cell refuses it, saying what to do.
    #[test]
    fn an_old_chronicle_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let mut old = serde_json::to_value(root(Z1)).unwrap();
        let o = old.as_object_mut().unwrap();
        o.remove("id");
        o.insert("case".into(), "opent".into());
        o.insert("zaakkenmerk".into(), Z1.into());
        std::fs::write(dir.path().join("test_kroniek.jsonl"), format!("{old}\n")).unwrap();
        let f = Chronicle::open(dir.path(), K).err().unwrap();
        assert!(f.contains("from before chronolex v0.2.0"), "{f}");
        assert!(f.contains("empty DATA_DIR"), "{f}");
    }

    /// A chronicle with an id twice is refused as a whole, also on a retry:
    /// the first attempt leaves nothing half loaded behind.
    #[test]
    fn a_duplicate_id_refuses_the_whole_chronicle_every_time() {
        let dir = tempfile::tempdir().unwrap();
        let row = serde_json::to_string(&root(Z1)).unwrap();
        std::fs::write(
            dir.path().join("test_kroniek.jsonl"),
            format!("{row}\n{row}\n"),
        )
        .unwrap();
        let k = Chronicle::open(dir.path(), &[]).unwrap();
        for _ in 0..2 {
            let f = k
                .add_provided(&root(Z2), K, |_, _| Ok::<(), ()>(()))
                .err()
                .unwrap();
            assert!(f.contains("occurs twice"), "{f}");
        }
    }

    #[test]
    fn append_only_earlier_lines_remain() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        k.add(&root(Z1)).unwrap();
        let path = dir.path().join("test_kroniek.jsonl");
        let before = std::fs::read_to_string(&path).unwrap();
        k.add(&root(Z2)).unwrap();
        let after = std::fs::read_to_string(&path).unwrap();
        assert!(after.starts_with(&before));
        assert_eq!(after.lines().count(), 2);
    }

    #[test]
    fn invalid_gram_is_not_recorded() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        let mut g = root(Z1);
        g.id = "geen-uuid".into();
        assert!(k.add(&g).unwrap_err().contains("id"));
        // A reference that is not a uuid, and an id that is already there.
        let mut g = root(Z1);
        g.refers_to.insert("application".into(), "geen-uuid".into());
        assert!(k.add(&g).is_err());
        k.add(&root(Z1)).unwrap();
        assert!(k.add(&root(Z1)).unwrap_err().contains("a gram with id"));
        assert_eq!(count(&k), 1);
    }

    #[test]
    fn a_gram_with_an_invalid_effective_at_is_not_recorded() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        let mut g = root(Z1);
        g.effective_at = "2025-03-12 10:14".into();
        let f = k.add(&g).unwrap_err();
        assert!(f.contains("effective_at '2025-03-12 10:14'"), "{f}");
        assert_eq!(count(&k), 0);
    }

    #[test]
    fn a_gram_without_recorded_at_is_not_recorded() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        let mut g = root(Z1);
        g.recorded_at = String::new();
        let f = k.add(&g).unwrap_err();
        assert!(f.contains("recorded_at"), "{f}");
        assert_eq!(count(&k), 0);
    }

    #[test]
    fn a_refusal_by_the_check_records_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        k.add(&root(Z1)).unwrap();
        let output = k
            .add_provided(&gram(Z1), K, |_, view| {
                if view.group.is_empty() {
                    Ok(())
                } else {
                    Err("something is already there")
                }
            })
            .unwrap();
        assert_eq!(output.err(), Some("something is already there"));
        assert_eq!(count(&k), 1);
    }

    /// The check sees the grams the gram refers to and the group of its root,
    /// from the indexes; a gram without a reference sees no group, and a gram
    /// with an unknown reference gets no root.
    #[test]
    fn the_check_sees_targets_and_group() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        k.add(&root(Z1)).unwrap();
        k.add(&root(Z2)).unwrap();
        k.add(&gram(Z1)).unwrap();
        let mut seen = (0, 0);
        k.add_provided(&gram(Z1), K, |g, view| {
            seen = (view.targets.len(), view.group.len());
            assert_eq!(g.root.as_deref(), Some(Z1));
            Ok::<(), ()>(())
        })
        .unwrap()
        .unwrap();
        assert_eq!(seen, (1, 2));
        let mut count_seen = usize::MAX;
        k.add_provided(&root(&uuid::Uuid::now_v7().to_string()), K, |_, view| {
            count_seen = view.group.len();
            Ok::<(), ()>(())
        })
        .unwrap()
        .unwrap();
        assert_eq!(count_seen, 0);
        let loose = gram("00000000-0000-4000-8000-00000000000f");
        let error = k.add_provided(&loose, K, |_, view| match &view.root_error {
            Some(f) => Err(f.clone()),
            None => Ok(()),
        });
        assert!(error.unwrap().unwrap_err().contains("no gram"));
    }

    #[test]
    fn concurrent_checks_let_one_through() {
        let dir = tempfile::tempdir().unwrap();
        let k = Arc::new(open(dir.path()));
        k.add(&root(Z1)).unwrap();
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let k = k.clone();
                std::thread::spawn(move || {
                    k.add_provided(&gram(Z1), K, |_, view| {
                        if view.group.len() == 1 {
                            Ok(())
                        } else {
                            Err(())
                        }
                    })
                    .unwrap()
                    .is_ok()
                })
            })
            .collect();
        let succeeded = threads
            .into_iter()
            .map(|d| d.join().unwrap())
            .filter(|ok| *ok)
            .count();
        assert_eq!(succeeded, 1);
        assert_eq!(count(&k), 2);
        let path = dir.path().join("test_kroniek.jsonl");
        assert_eq!(std::fs::read_to_string(path).unwrap().lines().count(), 2);
    }

    /// The `recorded_at` stamp happens under the write lock: with concurrent
    /// requests the order of the lines in the file is that of their
    /// `recorded_at`. The clock counts up per call here, but the threads reach
    /// the lock in arbitrary order; without the stamp under the lock, file and
    /// time would then diverge.
    #[test]
    fn the_stamp_happens_under_the_lock() {
        use std::sync::atomic::{AtomicI64, Ordering};
        let dir = tempfile::tempdir().unwrap();
        let k = Arc::new(open(dir.path()));
        let counter = Arc::new(AtomicI64::new(0));
        let threads: Vec<_> = (0..16)
            .map(|i| {
                let (k, counter) = (k.clone(), counter.clone());
                std::thread::spawn(move || {
                    let mut g = root(&format!("00000000-0000-4000-8000-{i:012}"));
                    g.recorded_at = "2000-01-01T00:00:00+01:00".into();
                    let clock = || {
                        let s = counter.fetch_add(1, Ordering::SeqCst);
                        DateTime::parse_from_rfc3339("2025-03-12T10:00:00+01:00").unwrap()
                            + chrono::Duration::seconds(s)
                    };
                    k.record_provided(g, &[], clock, |_| (), |_, _| Ok::<(), ()>(()))
                        .unwrap()
                        .unwrap()
                })
            })
            .collect();
        for d in threads {
            d.join().unwrap();
        }
        let text = std::fs::read_to_string(dir.path().join("test_kroniek.jsonl")).unwrap();
        let grams: Vec<Gram> = text
            .lines()
            .map(|r| serde_json::from_str::<Gram>(r).unwrap())
            .collect();
        let moments: Vec<String> = grams.iter().map(|g| g.recorded_at.clone()).collect();
        // Every gram got its own id under the lock.
        let mut ids: Vec<&str> = grams.iter().map(|g| g.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 16);
        assert_eq!(moments.len(), 16);
        let mut sorted = moments.clone();
        sorted.sort();
        assert_eq!(moments, sorted, "the file order is that of recorded_at");
        // Without a bound effective_at, the effective_at moves along.
        let g: Gram = serde_json::from_str(text.lines().next().unwrap()).unwrap();
        assert_eq!(g.effective_at, g.recorded_at);
    }

    /// If the clock runs backwards, a gram does not get an earlier
    /// `recorded_at` than the line before it. A bound `effective_at` after
    /// that stamp is refused.
    #[test]
    fn the_stamp_does_not_run_backwards() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        let t = |s: &str| DateTime::parse_from_rfc3339(s).unwrap();
        let first = k
            .record_provided(
                root(Z1),
                &[],
                || t("2025-03-12T10:00:00+01:00"),
                |f| f,
                |_, _| Ok::<(), String>(()),
            )
            .unwrap()
            .unwrap();
        let afterwards = k
            .record_provided(
                root(Z1),
                &[],
                || t("2025-03-12T09:00:00+01:00"),
                |f| f,
                |_, _| Ok::<(), String>(()),
            )
            .unwrap()
            .unwrap();
        assert_eq!(afterwards.gram.recorded_at, first.gram.recorded_at);
        let mut bound = root(Z1);
        bound.effective_at = "2025-03-13T00:00:00+01:00".into();
        bound.effective_at_legal_basis = Some(vec!["testregeling_aanvraag#1".into()]);
        bound.effective_at_stated = true;
        let f = k
            .record_provided(
                bound,
                &[],
                || t("2025-03-12T11:00:00+01:00"),
                |f| f,
                |_, _| Ok::<(), String>(()),
            )
            .unwrap()
            .unwrap_err();
        assert!(f.contains("after the recording"), "{f}");
        assert_eq!(count(&k), 2);
    }

    #[test]
    fn chronicle_name_is_not_a_path() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        assert!(k.read("../elders").is_err());
        assert!(Chronicle::open(dir.path(), &["../elders"]).is_err());
    }

    #[test]
    fn a_half_last_line_is_truncated_on_opening() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        k.add(&root(Z1)).unwrap();
        drop(k);
        // The runtime stopped in the middle of writing the second gram.
        let path = dir.path().join("test_kroniek.jsonl");
        let whole = std::fs::read_to_string(&path).unwrap();
        let second = serde_json::to_string(&root(Z2)).unwrap();
        std::fs::write(&path, format!("{whole}{}", &second[..40])).unwrap();

        let k = open(dir.path());
        assert_eq!(count(&k), 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), whole);
        // And after that recording simply continues, on a whole line.
        k.add(&root(Z2)).unwrap();
        drop(k);
        let k = open(dir.path());
        assert_eq!(count(&k), 2);
    }

    #[test]
    fn a_broken_line_in_the_middle_does_not_open() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_kroniek.jsonl");
        let g = serde_json::to_string(&root(Z1)).unwrap();
        std::fs::write(&path, format!("{g}\n{{\"kind\": \n{g}\n")).unwrap();
        let error = Chronicle::open(dir.path(), K).err().unwrap();
        assert!(error.contains("test_kroniek.jsonl line 2"), "{error}");
        // The file has not been touched.
        assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 3);
        // Not even when there is also a half last line: read first, only then
        // truncate.
        let with_tail = format!("{g}\n{{\"kind\": \n{g}\n{{\"ki");
        std::fs::write(&path, &with_tail).unwrap();
        assert!(Chronicle::open(dir.path(), K).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), with_tail);
    }

    #[test]
    fn a_leftover_of_a_failed_write_is_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        k.add(&root(Z1)).unwrap();
        // An earlier write left a half line behind that could not be rolled
        // back; the chronicle still knows the correct length.
        let path = dir.path().join("test_kroniek.jsonl");
        let whole = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, format!("{whole}{{\"half")).unwrap();
        k.add(&root(Z2)).unwrap();
        drop(k);
        let k = open(dir.path());
        assert_eq!(count(&k), 2);
    }

    #[test]
    fn leftovers_of_an_interrupted_initial_state_are_cleaned_up() {
        let dir = tempfile::tempdir().unwrap();
        let rest = dir.path().join("test_kroniek.jsonl.nieuw");
        std::fs::write(&rest, "{\"half").unwrap();
        let k = open(dir.path());
        assert!(!rest.exists());
        assert!(k.set_initial_state(K, &[root(Z1)]).unwrap());
        assert_eq!(count(&k), 1);
    }

    #[test]
    fn the_initial_state_in_one_go_and_only_in_an_empty_chronicle() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        let state = [root(Z1), root(Z2)];
        assert!(k.set_initial_state(K, &state).unwrap());
        assert_eq!(count(&k), 2);
        let path = dir.path().join("test_kroniek.jsonl");
        assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 2);
        assert!(!dir.path().join("test_kroniek.jsonl.nieuw").exists());
        // Not again.
        assert!(!k.set_initial_state(K, &state).unwrap());
        assert_eq!(count(&k), 2);
        // An invalid gram: nothing written.
        let empty = tempfile::tempdir().unwrap();
        let k = open(empty.path());
        let mut bad = root(Z2);
        bad.effective_at = "gisteren".into();
        assert!(k.set_initial_state(K, &[root(Z1), bad]).is_err());
        assert_eq!(count(&k), 0);
        assert!(!empty.path().join("test_kroniek.jsonl").exists());
        // A gram that refers to two roots: refused, not the last one taken.
        let mut both = gram(Z1);
        both.refers_to.insert("other".into(), Z2.into());
        let f = k
            .set_initial_state(K, &[root(Z1), root(Z2), both])
            .unwrap_err();
        assert!(f.contains("different roots"), "{f}");
        assert_eq!(count(&k), 0);
    }

    #[test]
    fn the_yaml_is_made_once() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        k.add(&root(Z1)).unwrap();
        let v = k.read("test_kroniek").unwrap().remove(0);
        let mut times = 0;
        for _ in 0..3 {
            let y = v
                .yaml(|g| {
                    times += 1;
                    Ok(g.name.clone())
                })
                .unwrap();
            assert_eq!(y, "melding_ontvangen");
        }
        assert_eq!(times, 1);
    }
}
