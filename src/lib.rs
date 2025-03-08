//! Run cron jobs manually.
//!
//! # Points of Interest
//!
//! - [`Crontab`](crontab::Crontab) The brain of the tool. It takes in
//!   crontab [`tokens`](tokens::Token), from which you can then run the
//!   jobs.
//! - [`make_instance()`](crontab::make_instance) Initialize a brand-new
//!   instance of [`Crontab`](crontab::Crontab) from the current user's
//!   crontab. It is API-sugar to read the crontab, parse it, and to
//!   feed it to [`Crontab`](crontab::Crontab).
//! - [`Crontab::jobs()`](crontab::Crontab::jobs) List all the
//!   [`jobs`](tokens::CronJob) available.
//! - [`Crontab::get_job_from_uid()`](crontab::Crontab::get_job_from_uid)
//!   get a job's instance from its UID.
//! - [`Crontab::run()`](crontab::Crontab::run) Run a job (taking into
//!   account any crontab variable defined before the job).
//! - [`Crontab::run_detached()`](crontab::Crontab::run_detached) Same
//!   as `run()`, but return instead of waiting for the job to finish.
//!
//! ## Lower level
//!
//! - [`Reader::read()`](reader::Reader::read) Read the current user's
//!   crontab to a `String`.
//! - [`Parser::parse()`](parser::Parser::parse) Parse a crontab
//!   `String` into crontab [`tokens`].

pub mod crontab;

pub use crontab::parser;
pub use crontab::reader;
pub use crontab::tokens;
