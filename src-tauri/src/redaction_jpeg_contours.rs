//! Bounded exact source-grid union outlines. Old saved paint grammars stay in
//! coverage.rs; only newly selected images use these merged boundaries.
use super::*;
use std::collections::BTreeSet;

type Point = [u32; 2];

fn direction(a: Point, b: Point) -> usize {
    if b[0] > a[0] {
        0
    } else if b[1] > a[1] {
        1
    } else if b[0] < a[0] {
        2
    } else {
        3
    }
}

impl Coverage {
    pub(super) fn contour_paint(&self, budget: &mut CoverageBudget) -> EngineResult<Paint> {
        let rectangles = self.rectangles(budget)?;
        if rectangles.is_empty() {
            return Ok(Paint {
                operations: vec![],
                regions: vec![],
            });
        }
        let s = self.shape;
        CoverageBudget::charge(&mut budget.cells, self.mask.len(), 8_000_000)?;
        let mut edges = BTreeSet::new();
        for y in 0..s.rows {
            for x in 0..s.columns {
                let index = (y * s.columns + x) as usize;
                if self.mask[index] == 0 {
                    continue;
                }
                let x0 = x * s.mcu_width;
                let x1 = ((x + 1) * s.mcu_width).min(s.width);
                let y0 = y * s.mcu_height;
                let y1 = ((y + 1) * s.mcu_height).min(s.height);
                let candidates = [
                    (
                        y == 0 || self.mask[index - s.columns as usize] == 0,
                        ([x0, y0], [x1, y0]),
                    ),
                    (
                        x + 1 == s.columns || self.mask[index + 1] == 0,
                        ([x1, y0], [x1, y1]),
                    ),
                    (
                        y + 1 == s.rows || self.mask[index + s.columns as usize] == 0,
                        ([x1, y1], [x0, y1]),
                    ),
                    (x == 0 || self.mask[index - 1] == 0, ([x0, y1], [x0, y0])),
                ];
                for (exposed, edge) in candidates {
                    if exposed {
                        // Bound retained edges before the tree allocates them.
                        CoverageBudget::charge(&mut budget.boundary_edges, 1, 100_000)?;
                        require(edges.insert(edge))?;
                    }
                }
            }
        }
        let mut contours = Vec::new();
        while let Some((mut a, mut b)) = edges.pop_first() {
            let first = a;
            let mut points = vec![a];
            while b != first {
                points.push(b);
                let prior = direction(a, b);
                // Keep diagonally touching components distinct: turn right
                // before straight/left at an ambiguous source-grid vertex.
                let next = edges
                    .range((b, [0, 0])..=(b, [u32::MAX, u32::MAX]))
                    .min_by_key(|(_, next)| match (direction(b, *next) + 4 - prior) % 4 {
                        1 => 0,
                        0 => 1,
                        3 => 2,
                        _ => 3,
                    })
                    .copied()
                    .ok_or_else(invalid)?;
                require(edges.remove(&next))?;
                a = b;
                b = next.1;
            }
            // Keep only direction changes, including the wrap-around vertex.
            // This is linear, unlike repeatedly deleting collinear vertices.
            let mut corners: Vec<Point> = points
                .iter()
                .enumerate()
                .filter_map(|(i, &p)| {
                    let previous = points[(i + points.len() - 1) % points.len()];
                    let next = points[(i + 1) % points.len()];
                    (direction(previous, p) != direction(p, next)).then_some(p)
                })
                .collect();
            require(corners.len() >= 4)?;
            corners.reverse(); // PDF counterclockwise exterior; opposite holes.
            let start = (0..corners.len())
                .min_by_key(|&i| (std::cmp::Reverse(corners[i][1]), corners[i][0]))
                .ok_or_else(invalid)?;
            corners.rotate_left(start);
            contours.push(corners);
        }
        let count = 4 + contours
            .iter()
            .map(|c| if c.len() == 4 { 8 } else { c.len() + 1 })
            .sum::<usize>();
        CoverageBudget::charge(&mut budget.operations, count, 100_000)?;
        let mut operations = Vec::with_capacity(count);
        operations.push(Operation::new("q", vec![]));
        operations.push(Operation::new("g", vec![Object::Integer(0)]));
        for contour in contours {
            let mut vertices = Vec::with_capacity(contour.len());
            for (i, &[x, y]) in contour.iter().enumerate() {
                let prev = contour[(i + contour.len() - 1) % contour.len()];
                let next = contour[(i + 1) % contour.len()];
                let vertical = if prev[0] == x {
                    y as i64 - prev[1] as i64
                } else {
                    next[1] as i64 - y as i64
                };
                let horizontal = if prev[1] == y {
                    x as i64 - prev[0] as i64
                } else {
                    next[0] as i64 - x as i64
                };
                vertices.push([
                    edge(x, s.width, vertical < 0),
                    edge(s.height - y, s.height, horizontal < 0),
                ]);
            }
            // A lone rectangle triggers PDFium's edge-trimming shortcut. Two
            // same-winding triangles cancel their diagonal in one nonzero fill.
            let paths = if vertices.len() == 4 {
                vec![vec![0, 1, 2], vec![0, 2, 3]]
            } else {
                vec![(0..vertices.len()).collect()]
            };
            for path in paths {
                for (i, index) in path.into_iter().enumerate() {
                    operations.push(Operation::new(
                        if i == 0 { "m" } else { "l" },
                        vertices[index].map(Object::Real).to_vec(),
                    ));
                }
                operations.push(Operation::new("h", vec![]));
            }
        }
        operations.push(Operation::new("f", vec![]));
        operations.push(Operation::new("Q", vec![]));
        let regions = rectangles
            .into_iter()
            .map(|[x0, y0, x1, y1]| AreaRect {
                left: serialized_edge(edge(x0, s.width, false), false),
                bottom: serialized_edge(edge(s.height - y1, s.height, false), false),
                right: serialized_edge(edge(x1, s.width, true), true),
                top: serialized_edge(edge(s.height - y0, s.height, true), true),
            })
            .collect();
        Ok(Paint {
            operations,
            regions,
        })
    }
}
