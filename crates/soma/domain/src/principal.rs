use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ScopeSet(BTreeSet<String>);

impl ScopeSet {
    pub fn new(scopes: impl IntoIterator<Item = String>) -> Self {
        Self(
            scopes
                .into_iter()
                .filter(|scope| !scope.is_empty())
                .collect(),
        )
    }

    pub fn contains(&self, scope: &str) -> bool {
        self.0.contains(scope)
    }

    pub fn to_vec(&self) -> Vec<String> {
        self.0.iter().cloned().collect()
    }
}

impl<const N: usize> From<[&str; N]> for ScopeSet {
    fn from(scopes: [&str; N]) -> Self {
        Self::new(scopes.into_iter().map(ToOwned::to_owned))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Principal {
    pub subject: String,
    pub scopes: ScopeSet,
    pub issuer: Option<String>,
}

impl Principal {
    pub fn new(subject: impl Into<String>, scopes: ScopeSet) -> Self {
        Self {
            subject: subject.into(),
            scopes,
            issuer: None,
        }
    }

    pub fn with_issuer(mut self, issuer: impl Into<String>) -> Self {
        self.issuer = Some(issuer.into());
        self
    }

    pub fn anonymous() -> Self {
        Self::new("anonymous", ScopeSet::default())
    }
}

#[cfg(test)]
mod tests {
    use super::ScopeSet;

    #[test]
    fn scope_sets_are_deduplicated_and_stable() {
        let scopes = ScopeSet::new([
            "soma:write".to_owned(),
            "soma:read".to_owned(),
            "soma:write".to_owned(),
        ]);
        assert_eq!(
            scopes.to_vec(),
            vec!["soma:read".to_owned(), "soma:write".to_owned()]
        );
    }
}
