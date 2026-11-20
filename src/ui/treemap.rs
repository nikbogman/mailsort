//! Treemap layout: blocks whose areas match their weights, filling a box.

/// A block's place inside the map, in percent of the map's width and height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

/// Lays out `weights` (any order) in a box `aspect` times wider than tall. Returns one rect per
/// weight, same order. Splits the list in two halves of near-equal weight along the longer side,
/// recursively, which keeps blocks close to square.
// ponytail: binary split, not squarified; good enough for ~20 blocks, swap in squarify if shapes get thin.
pub fn layout(weights: &[f64], aspect: f64) -> Vec<Rect> {
    let mut order: Vec<usize> = (0..weights.len()).collect();
    order.sort_by(|&a, &b| weights[b].total_cmp(&weights[a]));
    let mut out = vec![Rect { left: 0.0, top: 0.0, width: 0.0, height: 0.0 }; weights.len()];
    split(&order, weights, (0.0, 0.0, aspect, 1.0), &mut out);
    for r in &mut out {
        r.left = r.left / aspect * 100.0;
        r.width = r.width / aspect * 100.0;
        r.top *= 100.0;
        r.height *= 100.0;
    }
    out
}

fn split(items: &[usize], w: &[f64], (x, y, bw, bh): (f64, f64, f64, f64), out: &mut [Rect]) {
    match items {
        [] => {}
        [i] => out[*i] = Rect { left: x, top: y, width: bw, height: bh },
        _ => {
            let total: f64 = items.iter().map(|&i| w[i]).sum();
            let mut acc = 0.0;
            let mut cut = 1;
            for (k, &i) in items.iter().enumerate().take(items.len() - 1) {
                acc += w[i];
                cut = k + 1;
                if acc >= total / 2.0 {
                    break;
                }
            }
            let first: f64 = items[..cut].iter().map(|&i| w[i]).sum();
            let share = if total > 0.0 { first / total } else { 0.5 };
            if bw >= bh {
                split(&items[..cut], w, (x, y, bw * share, bh), out);
                split(&items[cut..], w, (x + bw * share, y, bw * (1.0 - share), bh), out);
            } else {
                split(&items[..cut], w, (x, y, bw, bh * share), out);
                split(&items[cut..], w, (x, y + bh * share, bw, bh * (1.0 - share)), out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn areas_match_weights_and_fill_the_map() {
        let weights = [312.0, 210.0, 186.0, 105.0, 97.0, 76.0, 61.0, 48.0, 39.0, 33.0, 24.0, 22.0, 18.0, 14.0];
        let rects = layout(&weights, 1.8);
        let total: f64 = weights.iter().sum();
        let mut covered = 0.0;
        for (r, w) in rects.iter().zip(weights) {
            let area = r.width * r.height;
            assert!((area / 10_000.0 - w / total).abs() < 1e-9, "{r:?} for {w}");
            assert!(r.left >= -1e-9 && r.top >= -1e-9 && r.left + r.width <= 100.0 + 1e-9 && r.top + r.height <= 100.0 + 1e-9);
            covered += area;
        }
        assert!((covered - 10_000.0).abs() < 1e-6);
    }

    #[test]
    fn single_block_fills_everything() {
        assert_eq!(layout(&[5.0], 2.0), vec![Rect { left: 0.0, top: 0.0, width: 100.0, height: 100.0 }]);
    }
}
