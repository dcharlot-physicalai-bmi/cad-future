//! End-to-end integration tests for the OpenIE CAD kernel.
//!
//! These tests exercise full workflows that cross crate boundaries:
//! parametric modeling → geometry → analysis → export → reimport.
//! Each test validates real engineering properties, not just "it runs."

#[cfg(test)]
mod parametric_workflow;

#[cfg(test)]
mod geometry_accuracy;

#[cfg(test)]
mod export_roundtrip;

#[cfg(test)]
mod simulation_pipeline;

#[cfg(test)]
mod sketch_to_solid;

#[cfg(test)]
mod manufacturing;

#[cfg(test)]
mod dfm_pipeline;

#[cfg(test)]
mod simulation_pipeline_extended;

#[cfg(test)]
mod export_pipeline;

#[cfg(test)]
mod material_cascade;

#[cfg(test)]
mod search_pipeline;

#[cfg(test)]
mod cascade_extended;

#[cfg(test)]
mod fea_validation;

#[cfg(test)]
mod impedance_gate;
