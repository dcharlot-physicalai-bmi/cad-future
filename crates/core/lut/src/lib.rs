#![cfg_attr(not(feature = "std"), no_std)]

//! LUT engine — the core of physical.openie.dev.
//!
//! LUT first. Formula second. Solver third. LLM fourth.
//! If the answer exists in a table, never compute it.

pub mod materials;
pub mod manufacturing;
pub mod formulas;
pub mod standards;
