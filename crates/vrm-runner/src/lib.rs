//! Conformance runner: reads test plans, drives renderer adapters, runs diff engine.

pub mod adapter;
pub mod cli;
pub mod diff;
pub mod execute;
pub mod execute_batch;
pub mod plan_to_ops;
