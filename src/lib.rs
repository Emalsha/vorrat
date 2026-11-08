//! Vorrat: a private, self-hosted emergency supply planner and household stock
//! tracker. One binary, one SQLite file, no telemetry.
//!
//! Shared modules (`db`, `web`, `auth`, `config`, `i18n`, `files`) name no
//! feature. Each feature folder (`stock`, `plan`, ...) holds its routes,
//! pages, queries and rules. See `docs/ARCHITECTURE.md`.

pub mod api;
pub mod auth;
pub mod config;
pub mod db;
pub mod demo;
pub mod essentials;
pub mod files;
pub mod home;
pub mod household;
pub mod i18n;
pub mod jobs;
pub mod locations;
pub mod lookup;
pub mod plan;
pub mod settings;
pub mod shopping;
pub mod smart;
pub mod stock;
pub mod transfer;
pub mod web;
