// SPDX-License-Identifier: GPL-3.0-or-later
//! OS-specific capture and desktop integration.
#[cfg(target_os = "linux")]
pub mod applications;
#[cfg(target_os = "linux")]
pub mod desktop;
#[cfg(feature = "imgur")]
pub mod imgur;
#[cfg(target_os = "linux")]
pub mod portal;
#[cfg(target_os = "linux")]
pub mod screens;
#[cfg(feature = "update-check")]
pub mod updates;
#[cfg(target_os = "linux")]
pub mod x11;

#[cfg(target_os = "linux")]
pub mod capture_lock;

#[cfg(target_os = "linux")]
pub mod clipboard;
