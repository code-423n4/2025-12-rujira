use itertools::{put_back, PutBack};
use std::{cmp::Ordering, iter::Fuse};

/// MergeByIter extended to support an arbitrary list of items, sorted and merged by a comparator
pub struct MergeNByIter<I: Iterator, F> {
    iters: Vec<PutBack<Fuse<I>>>,
    cmp_fn: F,
}

impl<I, F> MergeNByIter<I, F>
where
    I: Iterator,
    I::Item: Clone,
    F: Fn(&I::Item, &I::Item) -> Ordering,
{
    pub fn new<IntoIter>(iterators: impl IntoIterator<Item = IntoIter>, cmp_fn: F) -> Self
    where
        IntoIter: IntoIterator<Item = I::Item, IntoIter = I>,
    {
        Self {
            iters: iterators
                .into_iter()
                .map(|iter| put_back(iter.into_iter().fuse()))
                .collect(),
            cmp_fn,
        }
    }
}

impl<I, F> Iterator for MergeNByIter<I, F>
where
    I: Iterator,
    I::Item: Clone,
    F: Fn(&I::Item, &I::Item) -> Ordering,
{
    type Item = Vec<I::Item>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut best: Option<I::Item> = None;
        let mut items: Vec<(&mut PutBack<Fuse<I>>, I::Item)> = vec![];
        for lane in &mut self.iters {
            match (best.clone(), lane.next()) {
                (_, None) => continue,
                (None, Some(next)) => {
                    best = Some(next.clone());
                    items.push((lane, next));
                }
                (Some(b), Some(next)) => {
                    let cmp = (self.cmp_fn)(&b, &next);
                    match cmp {
                        Ordering::Greater => {
                            // If we discard values we need to put them back on their iterator
                            for i in &mut items {
                                i.0.put_back(i.1.clone());
                            }
                            best = Some(next.clone());
                            items = vec![(lane, next)];
                        }
                        Ordering::Equal => {
                            items.push((lane, next));
                        }
                        Ordering::Less => {
                            lane.put_back(next);
                        }
                    }
                }
            }
        }

        if items.is_empty() {
            None
        } else {
            Some(items.into_iter().map(|x| x.1).collect())
        }
    }
}

#[cfg(test)]
mod test {
    use proptest::prelude::{Strategy, *};

    use super::*;

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 10000,
            ..Default::default()
        })]
        #[test]
        fn test_ordering_is_correct(
            ranges in prop::collection::vec(
                (0i32..100i32, 0i32..100i32).prop_map(|(start, len)| start..(start + len)),
                1..10
            )
        ) {
            let vecs: Vec<Vec<i32>> = ranges.iter().map(|range| range.clone().collect()).collect();
            let iter = MergeNByIter::new(
                vecs.clone().into_iter(),
                |a: &i32, b: &i32| a.cmp(b)
            );
            let all = iter.collect::<Vec<Vec<i32>>>();
            let flattened: Vec<i32> = all.iter().flatten().copied().collect();

            prop_assert_eq!(vecs.concat().len(), flattened.len());
            for group in all {
                let first = group[0];
                for item in &group {
                    prop_assert_eq!(first, *item);
                }
            }
            for n in 1..flattened.len() {
            // We can just check that N=1 >= N on a flat list, as we've tested group membership above
                prop_assert!(flattened[n-1] <= flattened[n])
            }
        }
    }
}
