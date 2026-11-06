// SPDX-License-Identifier: GPL-3.0-or-later
//! OS-specific capture and desktop integration.
#[cfg(target_os = "linux")]
pub mod desktop;
#[cfg(target_os = "linux")]
pub mod portal;
#[cfg(target_os = "linux")]
pub mod screens;
#[cfg(target_os = "linux")]
pub mod x11;
