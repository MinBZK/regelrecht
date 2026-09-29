//! Sessions of the roles of a process (`roles` in `process.yaml`): every role
//! logs in through a simulated channel ([`crate::channel`]). One cookie per
//! process, one user per session: whoever logs in in another role replaces
//! the session. There is no register and no database check.
//!
//! A session expires after [`EXPIRE`] without use, and there are at most
//! [`MAXIMUM`] at once: whoever logs in then displaces the one unused the
//! longest. That way the memory does not grow with every login.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use axum::http::HeaderMap;

pub use crate::channel::Session as User;

/// Name of the session cookie.
pub const COOKIE: &str = "cell_session";

/// How long a session stays valid without use.
pub const EXPIRE: Duration = Duration::from_secs(8 * 60 * 60);

/// How many sessions a process has at once.
pub const MAXIMUM: usize = 10_000;

/// A session and when it was last used.
struct Active {
    user: User,
    last: Instant,
}

/// Sessions in memory: a restart logs everyone out.
pub struct Sessions {
    sessions: Mutex<HashMap<String, Active>>,
    expire: Duration,
    maximum: usize,
}

impl Default for Sessions {
    fn default() -> Self {
        Self::with(EXPIRE, MAXIMUM)
    }
}

impl Sessions {
    /// Sessions with their own expiry and maximum.
    pub fn with(expire: Duration, maximum: usize) -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            expire,
            maximum: maximum.max(1),
        }
    }

    /// The map; a thread that panicked under the lock leaves the sessions
    /// usable (nothing is ever half written).
    fn lock(&self) -> MutexGuard<'_, HashMap<String, Active>> {
        self.sessions.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn open(&self, user: User) -> String {
        self.open_at(user, Instant::now())
    }

    fn open_at(&self, user: User, now: Instant) -> String {
        let token = uuid::Uuid::new_v4().to_string();
        let mut m = self.lock();
        m.retain(|_, s| now.saturating_duration_since(s.last) < self.expire);
        while m.len() >= self.maximum {
            let Some(oldest) = m.iter().min_by_key(|(_, s)| s.last).map(|(t, _)| t.clone()) else {
                break;
            };
            m.remove(&oldest);
        }
        m.insert(token.clone(), Active { user, last: now });
        token
    }

    pub fn find(&self, headers: &HeaderMap) -> Option<User> {
        self.find_at(&token(headers)?, Instant::now())
    }

    /// The user of a session that is still valid; every use extends it.
    fn find_at(&self, token: &str, now: Instant) -> Option<User> {
        let mut m = self.lock();
        let s = m.get_mut(token)?;
        if now.saturating_duration_since(s.last) >= self.expire {
            m.remove(token);
            return None;
        }
        s.last = now;
        Some(s.user.clone())
    }

    pub fn remove(&self, headers: &HeaderMap) {
        if let Some(token) = token(headers) {
            self.lock().remove(&token);
        }
    }

    /// How many sessions there are now, expired or not.
    pub fn count(&self) -> usize {
        self.lock().len()
    }
}

fn token(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(axum::http::header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .find_map(|part| {
            let (name, value) = part.trim().split_once('=')?;
            (name == COOKIE).then(|| value.to_string())
        })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn user(role: &str, name: &str) -> User {
        User {
            role: role.into(),
            channel: "medewerker".into(),
            fields: [("naam".to_string(), name.to_string())].into(),
        }
    }

    fn employee(name: &str) -> User {
        user("behandelaar", name)
    }

    #[test]
    fn session_via_cookie() {
        let sessions = Sessions::default();
        let token = sessions.open(user("aanvrager", "A"));
        let mut h = HeaderMap::new();
        h.insert(
            axum::http::header::COOKIE,
            format!("ander=1; {COOKIE}={token}").parse().unwrap(),
        );
        assert_eq!(sessions.find(&h).unwrap().role, "aanvrager");
        assert_eq!(sessions.find(&h).unwrap().fields["naam"], "A");
        sessions.remove(&h);
        assert!(sessions.find(&h).is_none());
        assert!(sessions.find(&HeaderMap::new()).is_none());
    }

    #[test]
    fn a_session_expires_without_use() {
        let sessions = Sessions::with(Duration::from_secs(60), 100);
        let t0 = Instant::now();
        let token = sessions.open_at(employee("A"), t0);
        // Use extends it.
        assert!(sessions
            .find_at(&token, t0 + Duration::from_secs(50))
            .is_some());
        assert!(sessions
            .find_at(&token, t0 + Duration::from_secs(100))
            .is_some());
        // Then a minute of nothing: gone, and out of the map.
        assert!(sessions
            .find_at(&token, t0 + Duration::from_secs(161))
            .is_none());
        assert_eq!(sessions.count(), 0);
    }

    #[test]
    fn expired_sessions_are_cleaned_up_on_a_new_login() {
        let sessions = Sessions::with(Duration::from_secs(60), 100);
        let t0 = Instant::now();
        for i in 0..10 {
            sessions.open_at(employee(&format!("M{i}")), t0);
        }
        assert_eq!(sessions.count(), 10);
        sessions.open_at(employee("later"), t0 + Duration::from_secs(120));
        assert_eq!(sessions.count(), 1);
    }

    #[test]
    fn the_maximum_displaces_the_longest_unused() {
        let sessions = Sessions::with(Duration::from_secs(3600), 3);
        let t0 = Instant::now();
        let s = |i: u64| t0 + Duration::from_secs(i);
        let a = sessions.open_at(employee("A"), s(0));
        let b = sessions.open_at(employee("B"), s(1));
        let c = sessions.open_at(employee("C"), s(2));
        // A was just used, so B is the longest unused.
        assert!(sessions.find_at(&a, s(3)).is_some());
        let d = sessions.open_at(employee("D"), s(4));
        assert_eq!(sessions.count(), 3);
        assert!(sessions.find_at(&b, s(5)).is_none());
        for t in [&a, &c, &d] {
            assert!(sessions.find_at(t, s(5)).is_some());
        }
    }
}
