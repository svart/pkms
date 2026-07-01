use std::collections::{HashMap, HashSet, VecDeque};

use crate::domain::NoteId;
use crate::parser::Link;

use super::{Graph, NeighborSet};

impl Graph {
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
                if let Some(node) = self.nodes.get(uid) {
                    for link in &node.outgoing {
                        if let Link::Internal(target) = link
                            && visited.insert(target.clone())
                        {
                            next.push(target.clone());
                            if let Some(target_node) = self.nodes.get(target) {
                                neighbors.outgoing.push(target_node.clone());
                            } else {
                                neighbors.broken_outgoing.push(link.clone());
                            }
                        }
                    }
                }

                let primary = self.nodes.get(uid).map_or(uid, |n| &n.uuid);
                if let Some(backlinks) = self.backlinks.get(primary) {
                    for buid in backlinks {
                        if visited.insert(buid.clone()) {
                            next.push(buid.clone());
                            if let Some(back_node) = self.nodes.get(buid) {
                                neighbors.incoming.push(back_node.clone());
                            }
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

            if let Some(node) = self.nodes.get(&current) {
                for link in &node.outgoing {
                    if let Link::Internal(next) = link {
                        if next == &to_uuid {
                            let mut full = path.clone();
                            full.push(next.clone());
                            return Some(note_path_to_strings(full));
                        }
                        if visited.insert(next.clone()) {
                            let mut new_path = path.clone();
                            new_path.push(next.clone());
                            queue.push_back((next.clone(), new_path));
                        }
                    }
                }
            }

            let primary = self.nodes.get(&current).map_or(&current, |n| &n.uuid);
            if let Some(backlinks) = self.backlinks.get(primary) {
                for prev in backlinks {
                    if prev == &to_uuid {
                        let mut full = path.clone();
                        full.push(prev.clone());
                        return Some(note_path_to_strings(full));
                    }
                    if visited.insert(prev.clone()) {
                        let mut new_path = path.clone();
                        new_path.push(prev.clone());
                        queue.push_back((prev.clone(), new_path));
                    }
                }
            }
        }

        None
    }
}

fn note_path_to_strings(path: Vec<NoteId>) -> Vec<String> {
    path.into_iter().map(String::from).collect()
}
