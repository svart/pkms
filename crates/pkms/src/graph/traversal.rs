use std::collections::{HashMap, HashSet, VecDeque};

use crate::domain::NoteId;
use crate::parser::Link;

use super::{Graph, NeighborSet};

struct InternalOutgoing<'a> {
    target: &'a NoteId,
    link: &'a Link,
}

struct InternalAdjacency<'a> {
    outgoing: Vec<InternalOutgoing<'a>>,
    backlinks: &'a [NoteId],
}

impl Graph {
    fn internal_adjacency(&self, uuid: &NoteId) -> InternalAdjacency<'_> {
        let outgoing = self
            .nodes
            .get(uuid)
            .map(|node| {
                node.outgoing
                    .iter()
                    .filter_map(|link| match link {
                        Link::Internal(target) => Some(InternalOutgoing { target, link }),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let primary = self.nodes.get(uuid).map_or(uuid, |node| &node.uuid);
        let backlinks = self
            .backlinks
            .get(primary)
            .map(Vec::as_slice)
            .unwrap_or(&[]);

        InternalAdjacency {
            outgoing,
            backlinks,
        }
    }

    pub fn get_neighbors(&self, uuid: &str, max_depth: u32) -> HashMap<u32, NeighborSet> {
        let start = NoteId::new(uuid);
        let mut result = HashMap::new();
        let mut visited = HashSet::new();
        visited.insert(start.clone());

        let mut current = vec![start];
        for depth in 1..=max_depth {
            let mut neighbors = NeighborSet::default();
            let mut next = Vec::new();

            for uid in &current {
                let adjacency = self.internal_adjacency(uid);
                for outgoing in adjacency.outgoing {
                    if visited.insert(outgoing.target.clone()) {
                        next.push(outgoing.target.clone());
                        if let Some(target_node) = self.nodes.get(outgoing.target) {
                            neighbors.outgoing.push(target_node.clone());
                        } else {
                            neighbors.broken_outgoing.push(outgoing.link.clone());
                        }
                    }
                }

                for backlink in adjacency.backlinks {
                    if visited.insert(backlink.clone()) {
                        next.push(backlink.clone());
                        if let Some(back_node) = self.nodes.get(backlink) {
                            neighbors.incoming.push(back_node.clone());
                        }
                    }
                }
            }

            neighbors.outgoing.sort_by(|a, b| a.uuid.cmp(&b.uuid));
            neighbors.outgoing.dedup_by(|a, b| a.uuid == b.uuid);
            neighbors.incoming.sort_by(|a, b| a.uuid.cmp(&b.uuid));
            neighbors.incoming.dedup_by(|a, b| a.uuid == b.uuid);

            result.insert(depth, neighbors);
            current = next;
            if current.is_empty() {
                break;
            }
        }

        result
    }

    pub fn find_shortest_path(
        &self,
        from: &str,
        to: &str,
        max_depth: Option<u32>,
    ) -> Option<Vec<String>> {
        let from_uuid = self.find_node(from)?.uuid.clone();
        let to_uuid = self.find_node(to)?.uuid.clone();

        if from_uuid == to_uuid {
            return Some(vec![from_uuid.to_string()]);
        }

        let mut visited = HashSet::new();
        visited.insert(from_uuid.clone());
        let mut queue = VecDeque::new();
        queue.push_back((from_uuid.clone(), vec![from_uuid]));

        while let Some((current, path)) = queue.pop_front() {
            if max_depth.is_some_and(|md| path.len() > md as usize) {
                continue;
            }

            let adjacency = self.internal_adjacency(&current);
            for outgoing in adjacency.outgoing {
                if outgoing.target == &to_uuid {
                    let mut full = path.clone();
                    full.push(outgoing.target.clone());
                    return Some(note_path_to_strings(full));
                }
                if visited.insert(outgoing.target.clone()) {
                    let mut new_path = path.clone();
                    new_path.push(outgoing.target.clone());
                    queue.push_back((outgoing.target.clone(), new_path));
                }
            }

            for backlink in adjacency.backlinks {
                if backlink == &to_uuid {
                    let mut full = path.clone();
                    full.push(backlink.clone());
                    return Some(note_path_to_strings(full));
                }
                if visited.insert(backlink.clone()) {
                    let mut new_path = path.clone();
                    new_path.push(backlink.clone());
                    queue.push_back((backlink.clone(), new_path));
                }
            }
        }

        None
    }
}

fn note_path_to_strings(path: Vec<NoteId>) -> Vec<String> {
    path.into_iter().map(String::from).collect()
}
