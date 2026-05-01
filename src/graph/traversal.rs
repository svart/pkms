use std::collections::{HashMap, HashSet, VecDeque};

use crate::parser::Link;

use super::{Graph, NeighborSet, Subgraph};

impl Graph {
    pub fn get_neighbors(&self, uuid: &str, max_depth: u32) -> HashMap<u32, NeighborSet> {
        let mut result = HashMap::new();
        let mut visited = HashSet::new();
        visited.insert(uuid.to_string());

        let mut current = vec![uuid.to_string()];
        for depth in 1..=max_depth {
            let mut neighbors = NeighborSet::default();
            let mut next = Vec::new();

            for uid in &current {
                if let Some(node) = self.nodes.get(uid) {
                    for link in &node.outgoing {
                        if let Link::Internal(target) = link {
                            if visited.insert(target.clone()) {
                                next.push(target.clone());
                            }
                            if let Some(target_node) = self.nodes.get(target) {
                                neighbors.outgoing.push(target_node.clone());
                            } else {
                                neighbors.broken_outgoing.push(link.clone());
                            }
                        }
                    }
                }

                if let Some(backlinks) = self.backlinks.get(uid) {
                    for buid in backlinks {
                        if visited.insert(buid.clone()) {
                            next.push(buid.clone());
                        }
                        if let Some(back_node) = self.nodes.get(buid) {
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
            return Some(vec![from_uuid]);
        }

        let mut visited = HashSet::new();
        visited.insert(from_uuid.clone());
        let mut queue = VecDeque::new();
        queue.push_back((from_uuid.clone(), vec![from_uuid]));

        while let Some((current, path)) = queue.pop_front() {
            if max_depth.map_or(false, |md| path.len() as u32 > md) {
                continue;
            }

            if let Some(node) = self.nodes.get(&current) {
                for link in &node.outgoing {
                    if let Link::Internal(next) = link {
                        if next == &to_uuid {
                            let mut full = path.clone();
                            full.push(next.clone());
                            return Some(full);
                        }
                        if visited.insert(next.clone()) {
                            let mut new_path = path.clone();
                            new_path.push(next.clone());
                            queue.push_back((next.clone(), new_path));
                        }
                    }
                }
            }

            if let Some(backlinks) = self.backlinks.get(&current) {
                for prev in backlinks {
                    if prev == &to_uuid {
                        let mut full = path.clone();
                        full.push(prev.clone());
                        return Some(full);
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

    pub fn collect_subgraph(&self, root: &str, max_depth: u32) -> Subgraph {
        let mut nodes_map: HashMap<String, super::Node> = HashMap::new();
        let mut edges = Vec::new();
        let mut visited = HashSet::new();
        let root_uuid = match self.find_node(root) {
            Some(n) => n.uuid.clone(),
            None => return Subgraph::empty(),
        };

        let mut current = vec![root_uuid.clone()];
        visited.insert(root_uuid.clone());

        if let Some(root_node) = self.nodes.get(&root_uuid) {
            nodes_map.insert(root_uuid.clone(), root_node.clone());
        }

        for _depth in 1..=max_depth {
            let mut next = Vec::new();

            for uid in &current {
                if let Some(node) = self.nodes.get(uid) {
                    for link in &node.outgoing {
                        if let Link::Internal(target) = link {
                            edges.push((uid.clone(), target.clone()));
                            if visited.insert(target.clone()) {
                                next.push(target.clone());
                                if let Some(target_node) = self.nodes.get(target) {
                                    nodes_map.insert(target.clone(), target_node.clone());
                                }
                            }
                        }
                    }
                }

                if let Some(back) = self.backlinks.get(uid) {
                    for buid in back {
                        edges.push((buid.clone(), uid.clone()));
                        if visited.insert(buid.clone()) {
                            next.push(buid.clone());
                            if let Some(back_node) = self.nodes.get(buid) {
                                nodes_map.insert(buid.clone(), back_node.clone());
                            }
                        }
                    }
                }
            }

            current = next;
            if current.is_empty() {
                break;
            }
        }

        edges.sort();
        edges.dedup();

        let vertex_count = nodes_map.len();
        let edge_count = edges.len();
        let avg_order = if vertex_count > 0 {
            edge_count as f64 / vertex_count as f64
        } else {
            0.0
        };

        let mut sorted_nodes: Vec<&super::Node> = nodes_map.values().collect();
        sorted_nodes.sort_by(|a, b| a.uuid.cmp(&b.uuid));

        Subgraph {
            root_uuid,
            nodes: sorted_nodes.into_iter().map(|n| n.clone()).collect(),
            edges,
            vertex_count,
            edge_count,
            avg_vertex_order: avg_order,
        }
    }
}
