// cronrunner — Run cron jobs manually.
// Copyright (C) 2024  Quentin Richert
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

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
