//! Conformance runner: reads test plans, drives renderer adapters, runs diff engine.

pub mod adapter;
pub mod benchmark;
pub mod blank;
pub mod cli;
pub mod diff;
pub mod execute;
pub mod execute_batch;
pub mod execute_matrix;
pub mod penetration_diff;
pub mod plan_to_ops;
