//! Search pipeline tests — Design → Index → Search.
//!
//! Creates multiple parts with distinct geometries, indexes them in HyperDB
//! via physical-search, and verifies that similarity search returns the
//! correct part as the top match.

use std::collections::HashMap;
use physical_brep::builder::{make_box, make_cylinder};
use physical_brep::Profile;
use physical_search::PartIndex;

/// Create 3 parts (box, cylinder, L-bracket), index all, search for similar to box → box ranks first.
#[test]
fn search_box_ranks_first() {
    let index = PartIndex::new();

    // Create distinct geometries
    let box_solid = make_box(50.0, 30.0, 20.0);
    let cylinder_solid = make_cylinder(15.0, 40.0, 16);
    let l_bracket = {
        let profile = Profile::l_shape(30.0, 20.0, 5.0);
        physical_brep::extrude::extrude_z(&profile, 10.0)
    };

    // Index all parts
    index.index_part(&box_solid, "box", None, HashMap::new());
    index.index_part(&cylinder_solid, "cylinder", None, HashMap::new());
    index.index_part(&l_bracket, "l-bracket", None, HashMap::new());

    // Search for similar to box
    let results = index.search_similar(&box_solid, None, 3);

    assert!(
        !results.is_empty(),
        "search should return at least one result"
    );
    assert_eq!(
        results[0].part_name, "box",
        "searching with box query should return 'box' first, got '{}'",
        results[0].part_name
    );

    // Top result should have high similarity (low distance → high score)
    assert!(
        results[0].similarity_score > 0.5,
        "top result similarity {:.3} should be high (> 0.5)",
        results[0].similarity_score
    );
}

/// Search for similar to cylinder → cylinder ranks first.
#[test]
fn search_cylinder_ranks_first() {
    let index = PartIndex::new();

    let box_solid = make_box(50.0, 30.0, 20.0);
    let cylinder_solid = make_cylinder(15.0, 40.0, 16);
    let l_bracket = {
        let profile = Profile::l_shape(30.0, 20.0, 5.0);
        physical_brep::extrude::extrude_z(&profile, 10.0)
    };

    index.index_part(&box_solid, "box", None, HashMap::new());
    index.index_part(&cylinder_solid, "cylinder", None, HashMap::new());
    index.index_part(&l_bracket, "l-bracket", None, HashMap::new());

    let results = index.search_similar(&cylinder_solid, None, 3);

    assert!(
        !results.is_empty(),
        "search should return results"
    );
    assert_eq!(
        results[0].part_name, "cylinder",
        "searching with cylinder query should return 'cylinder' first, got '{}'",
        results[0].part_name
    );
}

/// Search for similar to L-bracket → L-bracket ranks first.
#[test]
fn search_l_bracket_ranks_first() {
    let index = PartIndex::new();

    let box_solid = make_box(50.0, 30.0, 20.0);
    let cylinder_solid = make_cylinder(15.0, 40.0, 16);
    let l_bracket = {
        let profile = Profile::l_shape(30.0, 20.0, 5.0);
        physical_brep::extrude::extrude_z(&profile, 10.0)
    };

    index.index_part(&box_solid, "box", None, HashMap::new());
    index.index_part(&cylinder_solid, "cylinder", None, HashMap::new());
    index.index_part(&l_bracket, "l-bracket", None, HashMap::new());

    let results = index.search_similar(&l_bracket, None, 3);

    assert!(
        !results.is_empty(),
        "search should return results"
    );
    assert_eq!(
        results[0].part_name, "l-bracket",
        "searching with L-bracket query should return 'l-bracket' first, got '{}'",
        results[0].part_name
    );
}

/// Feature extraction produces distinct vectors for distinct geometries.
#[test]
fn feature_vectors_are_distinct() {
    let box_solid = make_box(50.0, 30.0, 20.0);
    let cylinder_solid = make_cylinder(15.0, 40.0, 16);

    let box_fv = physical_search::extract_features(&box_solid, None);
    let cyl_fv = physical_search::extract_features(&cylinder_solid, None);

    // At least some dimensions should differ
    let mut diff_count = 0;
    for i in 0..physical_search::FEATURE_DIM {
        if (box_fv.data[i] - cyl_fv.data[i]).abs() > 0.01 {
            diff_count += 1;
        }
    }

    assert!(
        diff_count >= 3,
        "box and cylinder feature vectors should differ in at least 3 dimensions, got {diff_count}"
    );
}
