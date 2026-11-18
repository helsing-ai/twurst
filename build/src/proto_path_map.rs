//! Maps a fully-qualified Protobuf path to a value using path matchers.
//! Original implementation: <https://github.com/tokio-rs/prost/blob/5a2c7092964ac2eaaa516c61bcd48e3c66ea16b3/prost-build/src/path.rs>

use prost_build::Service;
use std::iter;

#[derive(Clone, Debug, Default)]
pub(crate) struct ProtoPathMap<T> {
    pub(crate) matchers: Vec<(String, T)>,
}

impl<T> ProtoPathMap<T> {
    /// Inserts a new matcher and associated value
    pub(crate) fn insert(&mut self, matcher: String, value: T) {
        self.matchers.push((matcher, value));
    }

    /// Returns a iterator over all the values matching the given service
    pub(crate) fn service_matches(
        &self,
        service: &Service,
    ) -> impl Iterator<Item = &T> + use<'_, T> {
        self.fq_path_matches(&format!(".{}.{}", service.package, service.proto_name))
    }

    /// Returns a iterator over all the values matching the given fully-qualified proto path
    pub(crate) fn fq_path_matches(&self, fq_path: &str) -> impl Iterator<Item = &T> + use<'_, T> {
        let fq_path = fq_path.to_owned();
        self.matchers
            .iter()
            .filter(move |(matcher, _)| sub_path_iter(&fq_path).any(|p| p == matcher))
            .map(|(_, value)| value)
    }
}

/// Given a fully-qualified path, returns a sequence of paths:
/// - the path itself
/// - the sequence of suffix paths
/// - the sequence of prefix paths
/// - the global path
///
/// Example: sub_path_iter(".a.b.c") -> [".a.b.c", "a.b.c", "b.c", "c", ".a.b", ".a", "."]
fn sub_path_iter(full_path: &str) -> impl Iterator<Item = &str> {
    // First, try matching the path.
    iter::once(full_path)
        // Then, try matching path suffixes.
        .chain(suffixes(full_path))
        // Then, try matching path prefixes.
        .chain(prefixes(full_path))
        // Then, match the global path.
        .chain(iter::once("."))
}

/// Given a fully-qualified path, returns a sequence of fully-qualified paths which match a prefix
/// of the input path, in decreasing path-length order.
///
/// Example: prefixes(".a.b.c.d") -> [".a.b.c", ".a.b", ".a"]
fn prefixes(fq_path: &str) -> impl Iterator<Item = &str> {
    std::iter::successors(Some(fq_path), |path| {
        #[allow(unknown_lints, clippy::manual_split_once)]
        path.rsplitn(2, '.').nth(1).filter(|path| !path.is_empty())
    })
    .skip(1)
}

/// Given a fully-qualified path, returns a sequence of paths which match the suffix of the input
/// path, in decreasing path-length order.
///
/// Example: suffixes(".a.b.c.d") -> ["a.b.c.d", "b.c.d", "c.d", "d"]
fn suffixes(fq_path: &str) -> impl Iterator<Item = &str> {
    std::iter::successors(Some(fq_path), |path| {
        #[allow(unknown_lints, clippy::manual_split_once)]
        path.splitn(2, '.').nth(1).filter(|path| !path.is_empty())
    })
    .skip(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost_build::Comments;

    impl<T> ProtoPathMap<T> {
        fn clear(&mut self) {
            self.matchers.clear();
        }
    }

    #[test]
    fn test_prefixes() {
        assert_eq!(
            prefixes(".a.b.c.d").collect::<Vec<_>>(),
            vec![".a.b.c", ".a.b", ".a"],
        );
        assert_eq!(prefixes(".a").count(), 0);
        assert_eq!(prefixes(".").count(), 0);
    }

    #[test]
    fn test_suffixes() {
        assert_eq!(
            suffixes(".a.b.c.d").collect::<Vec<_>>(),
            vec!["a.b.c.d", "b.c.d", "c.d", "d"],
        );
        assert_eq!(suffixes(".a").collect::<Vec<_>>(), vec!["a"]);
        assert_eq!(suffixes(".").collect::<Vec<_>>(), Vec::<&str>::new());
    }

    #[test]
    fn test_get_matches_sub_path() {
        let mut path_map = ProtoPathMap::default();

        // full path
        path_map.insert(".a.b.c.d".to_owned(), 1);
        assert_eq!(Some(&1), path_map.fq_path_matches(".a.b.c.d").next());

        // suffix
        path_map.clear();
        path_map.insert("c.d".to_owned(), 1);
        assert_eq!(Some(&1), path_map.fq_path_matches(".a.b.c.d").next());
        assert_eq!(Some(&1), path_map.fq_path_matches("b.c.d").next());

        // prefix
        path_map.clear();
        path_map.insert(".a.b".to_owned(), 1);
        assert_eq!(Some(&1), path_map.fq_path_matches(".a.b.c.d").next());

        // global
        path_map.clear();
        path_map.insert(".".to_owned(), 1);
        assert_eq!(Some(&1), path_map.fq_path_matches(".a.b.c.d").next());
        assert_eq!(Some(&1), path_map.fq_path_matches("b.c.d").next());
    }

    #[test]
    fn test_get_keep_order() {
        let mut path_map = ProtoPathMap::default();
        path_map.insert(".".to_owned(), 1);
        path_map.insert(".a.b".to_owned(), 2);
        path_map.insert(".a.b.c.d".to_owned(), 3);

        assert_eq!(
            path_map.fq_path_matches(".a.b.c.d").collect::<Vec<_>>(),
            [&1, &2, &3]
        );

        path_map.clear();

        path_map.insert(".a.b.c.d".to_owned(), 1);
        path_map.insert(".a.b".to_owned(), 2);
        path_map.insert(".".to_owned(), 3);

        assert_eq!(
            path_map.fq_path_matches(".a.b.c.d").collect::<Vec<_>>(),
            [&1, &2, &3]
        );
    }

    #[test]
    fn test_service_matches() {
        // matches paths & all prefixes
        let mut path_map = ProtoPathMap::default();
        path_map.insert(".".to_owned(), 1);
        path_map.insert(".a.b".to_owned(), 2);
        path_map.insert(".a.b.c.d".to_owned(), 3);

        let service = Service {
            proto_name: "d".to_string(),
            package: "a.b.c".to_string(),
            name: "d".to_string(),
            comments: Comments::default(),
            methods: Vec::new(),
            options: prost_types::ServiceOptions {
                deprecated: None,
                uninterpreted_option: Vec::new(),
            },
        };

        assert_eq!(
            path_map.service_matches(&service).collect::<Vec<_>>(),
            [&1, &2, &3]
        );

        path_map.clear();

        // matches just on service name
        path_map.insert("d".to_owned(), 1);

        assert_eq!(path_map.service_matches(&service).collect::<Vec<_>>(), [&1]);

        path_map.clear();

        // matches just on path (and others do not)
        path_map.insert(".a.b.c.d".to_owned(), 1);

        assert_eq!(path_map.service_matches(&service).collect::<Vec<_>>(), [&1]);

        let alternate_service = Service {
            proto_name: "e".to_string(),
            package: "a.b.c".to_string(),
            name: "e".to_string(),
            comments: Comments::default(),
            methods: Vec::new(),
            options: prost_types::ServiceOptions {
                deprecated: None,
                uninterpreted_option: Vec::new(),
            },
        };

        assert_eq!(path_map.service_matches(&alternate_service).next(), None);
    }
}
