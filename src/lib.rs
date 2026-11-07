//! Vorrat: a private, self-hosted emergency supply planner and household stock
//! tracker. One binary, one SQLite file, no telemetry.

pub mod auth;
pub mod backup;
pub mod config;
pub mod db;
pub mod demo;
pub mod files;
pub mod i18n;
pub mod lookup;
pub mod notify;
pub mod rules;
pub mod scheduler;
pub mod smart;
pub mod transfer;
pub mod web;
