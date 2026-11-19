//! Bounded background ownership resolution, independent of filesystem lanes.
mod cache;
mod pool;
mod view;
use super::{Name, Ownership, read_sid, resolve_name};
use crate::{
    app::App,
    fs::{Entry, Identity},
    ui::Geometry,
};
use cache::Cache;
use pool::Pool;
use std::{
    hash::{Hash, Hasher},
    io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

type Answer = Result<String, String>;

/// One UI-owned service. Four fixed workers never share the listing runtime.
pub struct Runtime {
    readers: Pool<Key, Answer>,
    namers: Pool<String, Name>,
    paths: Cache<Key, Ownership>,
    names: Cache<String, Name>,
    generations: [u64; 2],
    contexts: [u64; 2],
    epoch: u64,
}

// Contents/access timestamps do not select another owner. Keep the original
// identity for native validation, but coalesce the same object across surfaces.
#[derive(Clone, Debug)]
struct Key {
    path: PathBuf,
    identity: Identity,
}
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
            && self.identity.object == other.identity.object
            && self.identity.created == other.identity.created
    }
}
impl Eq for Key {}
impl Hash for Key {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.path.hash(state);
        self.identity.object.hash(state);
        self.identity.created.hash(state);
    }
}
impl Key {
    fn new(entry: &Entry) -> Option<Self> {
        if entry.owner.is_some() {
            return None;
        }
        let identity = entry
            .identity
            .as_ref()
            .filter(|identity| identity.object.is_some())?;
        Some(Self {
            path: entry.path.clone(),
            identity: identity.clone(),
        })
    }
    fn matches(&self, entry: &Entry) -> bool {
        entry.path == self.path
            && entry.identity.as_ref().is_some_and(|identity| {
                identity.object == self.identity.object && identity.created == self.identity.created
            })
    }
}

impl Runtime {
    /// Start fixed read/name workers without performing ownership queries.
    ///
    /// # Errors
    /// Reports worker thread allocation failures before entering the UI loop.
    pub fn new() -> io::Result<Self> {
        Self::with_backend(
            |path, identity| read_sid(path, identity).map_err(|error| error.to_string()),
            resolve_name,
        )
    }

    fn with_backend(
        read: impl Fn(&Path, &Identity) -> Answer + Send + Sync + 'static,
        name: impl Fn(&str) -> Name + Send + Sync + 'static,
    ) -> io::Result<Self> {
        Ok(Self {
            readers: Pool::new("sid", move |key: &Key| read(&key.path, &key.identity))?,
            namers: Pool::new("name", move |sid: &String| name(sid))?,
            paths: Cache::new(512, Duration::from_secs(30)),
            names: Cache::new(256, Duration::from_secs(300)),
            generations: [0; 2],
            contexts: [0; 2],
            epoch: 0,
        })
    }

    #[cfg(test)]
    fn using(
        read: impl Fn(&Path, &Identity) -> Answer + Send + Sync + 'static,
        name: impl Fn(&str) -> Name + Send + Sync + 'static,
    ) -> Self {
        Self::with_backend(read, name).unwrap()
    }

    /// Schedule only requested metadata and rendered owners. Never wait for a worker.
    /// Obsolete native calls retain their slot until they finish; cancellation
    /// cannot forcibly interrupt a kernel or domain lookup.
    pub fn poll(&mut self, app: &mut App, geometry: &Geometry) -> bool {
        self.synchronize(app);
        let current = view::current(app, geometry);
        let wanted: Vec<_> = current.iter().map(|target| target.key.clone()).collect();
        self.readers.cancel_except(&wanted, self.epoch);
        let now = Instant::now();
        self.collect(now);
        let listing = app.panes.iter().any(|pane| pane.listing);
        let mut changed = false;
        for target in current {
            if target.requested
                && self.paths.get(&target.key, now).is_none()
                && !listing
                && let Err(error) = self.readers.start(target.key.clone(), self.epoch)
            {
                let owner = Ownership::Unavailable(error);
                self.paths.insert(target.key.clone(), owner, now);
            }
            if let Some(owner) = self.cached(
                &target.key,
                target.known(app),
                target.requested && target.names,
                now,
            ) {
                changed |= view::apply(app, &target, owner);
            }
        }
        changed
    }

    fn synchronize(&mut self, app: &App) {
        let contexts = [app.panes[0].context, app.panes[1].context];
        if self.contexts != contexts {
            self.contexts = contexts;
            self.paths.clear();
        }
        let generations = [app.panes[0].generation, app.panes[1].generation];
        if self.generations != generations {
            self.generations = generations;
            self.epoch = self.epoch.wrapping_add(1);
            self.paths.invalidate();
            self.readers.cancel_except(&[], self.epoch);
        }
    }

    fn collect(&mut self, now: Instant) {
        for reply in self.readers.take() {
            if reply.epoch != self.epoch {
                continue;
            }
            if let Some(answer) = reply.value {
                let owner = match answer.and_then(|answer| answer) {
                    Ok(sid) => Ownership::Sid {
                        sid,
                        name: Name::Pending,
                    },
                    Err(error) => Ownership::Unavailable(error),
                };
                self.paths.insert(reply.key, owner, now);
            }
        }
        for reply in self.namers.take() {
            if let Some(answer) = reply.value {
                self.names
                    .insert(reply.key, answer.unwrap_or_else(Name::Failed), now);
            }
        }
    }

    fn cached(
        &mut self,
        key: &Key,
        known: &Ownership,
        names: bool,
        now: Instant,
    ) -> Option<Ownership> {
        let mut owner = self
            .paths
            .peek(key)
            .or_else(|| (!matches!(known, Ownership::Pending)).then_some(known))
            .cloned()?;
        if let Ownership::Sid { sid, name } = &mut owner {
            if matches!(name, Name::Pending) {
                *name = self.previous_name(sid, known);
            }
            if let Some(cached) = self.names.get(sid, now) {
                *name = cached.clone();
            } else if names && let Err(error) = self.namers.start(sid.clone(), 0) {
                *name = Name::Failed(error);
                self.names.insert(sid.clone(), name.clone(), now);
            }
        }
        Some(owner)
    }

    fn previous_name(&self, sid: &str, known: &Ownership) -> Name {
        if let Ownership::Sid {
            sid: known_sid,
            name,
        } = known
            && sid == known_sid
            && !matches!(name, Name::Pending)
        {
            return name.clone();
        }
        self.names
            .peek(&sid.to_owned())
            .cloned()
            .unwrap_or(Name::Pending)
    }
}

#[cfg(test)]
mod tests;
