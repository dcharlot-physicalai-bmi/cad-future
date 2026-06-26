//! Tab (hold-down) generation for laser cutting.
//!
//! Tabs are small uncut sections along contour paths that hold parts
//! in place during cutting, preventing them from shifting or falling.

use glam::DVec2;
use physical_mfg_toolpath::Contour;

/// A tab location along a contour.
#[derive(Clone, Debug)]
pub struct Tab {
    /// Index into the contour's point list (tab starts between point[index] and point[index+1]).
    pub segment_index: usize,
    /// Parameter along the segment (0.0 = start, 1.0 = end).
    pub t: f64,
    /// Width of the tab in mm.
    pub width: f64,
}

/// Generate tab locations along a contour at regular intervals.
pub fn generate_tabs(contour: &Contour, tab_spacing: f64, tab_width: f64) -> Vec<Tab> {
    if !contour.is_closed || contour.points.len() < 3 || tab_spacing <= 0.0 {
        return Vec::new();
    }

    let perimeter = contour.length();
    let tab_count = (perimeter / tab_spacing).round().max(2.0) as usize;
    let actual_spacing = perimeter / tab_count as f64;

    let mut tabs = Vec::new();
    let mut accumulated = actual_spacing / 2.0; // Start half-spacing in

    // Walk along the contour
    let n = contour.points.len();
    let mut distance_so_far = 0.0;
    let mut tab_idx = 0;

    for i in 0..n {
        let j = (i + 1) % n;
        let seg_len = (contour.points[j] - contour.points[i]).length();

        while tab_idx < tab_count && accumulated <= distance_so_far + seg_len {
            let t = (accumulated - distance_so_far) / seg_len;
            tabs.push(Tab {
                segment_index: i,
                t: t.clamp(0.0, 1.0),
                width: tab_width,
            });
            tab_idx += 1;
            accumulated += actual_spacing;
        }

        distance_so_far += seg_len;
    }

    tabs
}

/// Split a contour into segments, leaving gaps where tabs are placed.
/// Returns a list of contour segments (open paths) between tab locations.
pub fn apply_tabs(contour: &Contour, tabs: &[Tab]) -> Vec<Vec<DVec2>> {
    if tabs.is_empty() || contour.points.is_empty() {
        return vec![contour.points.clone()];
    }

    let n = contour.points.len();
    let mut segments: Vec<Vec<DVec2>> = Vec::new();
    let mut current_segment: Vec<DVec2> = Vec::new();

    for i in 0..n {
        let j = (i + 1) % n;
        current_segment.push(contour.points[i]);

        // Check if any tab falls on this edge
        for tab in tabs {
            if tab.segment_index == i {
                let p0 = contour.points[i];
                let p1 = contour.points[j];
                let edge_dir = (p1 - p0).normalize();
                let edge_len = (p1 - p0).length();

                let tab_center = tab.t * edge_len;
                let tab_start = (tab_center - tab.width / 2.0).max(0.0);
                let tab_end = (tab_center + tab.width / 2.0).min(edge_len);

                // End current segment at tab start
                let tab_start_pt = p0 + edge_dir * tab_start;
                current_segment.push(tab_start_pt);
                if current_segment.len() >= 2 {
                    segments.push(current_segment);
                }

                // Start new segment after tab
                current_segment = Vec::new();
                let tab_end_pt = p0 + edge_dir * tab_end;
                current_segment.push(tab_end_pt);
            }
        }
    }

    // Close: add remaining segment
    if !current_segment.is_empty() {
        if let Some(first_seg) = segments.first_mut() {
            // Append to first segment to close the loop
            current_segment.extend(first_seg.iter());
            *first_seg = current_segment;
        } else {
            segments.push(current_segment);
        }
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_generation() {
        let c = Contour::closed(vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(40.0, 0.0),
            DVec2::new(40.0, 40.0),
            DVec2::new(0.0, 40.0),
        ]);
        let tabs = generate_tabs(&c, 40.0, 3.0);
        // Perimeter = 160, spacing = 40 → 4 tabs
        assert_eq!(tabs.len(), 4);
    }

    #[test]
    fn tabs_split_contour() {
        let c = Contour::closed(vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(100.0, 0.0),
            DVec2::new(100.0, 100.0),
            DVec2::new(0.0, 100.0),
        ]);
        let tabs = generate_tabs(&c, 100.0, 5.0);
        let segments = apply_tabs(&c, &tabs);
        assert!(segments.len() >= 2, "Tabs should split contour into segments");
    }
}
